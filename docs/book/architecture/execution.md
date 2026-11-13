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

A certificate for the complete original theory can select a class-specific
membership check. The CPU supports tight, positive and stratified plans.
The GPU supports the tight plan. For normal rules,
the answer-set condition is equality with the least consequences of the
candidate's reduct together with satisfaction of its constraints.

The general membership check rejects a proposed interpretation `M` that
fails the original theory `T`. Candidate-only restrictions further select
which models the session seeks. For each eligible `M`, the inner search
asks whether a proper subset `J` of `M` satisfies the frozen reduct
`T^M`. A subset can be a countermodel even
when it fails a candidate-only restriction. A validated countermodel
rejects `M`. Complete coverage without one establishes that `M` is an
answer set of `T`. An interrupted search without a witness leaves the
question open. Each candidate has its own frozen reduct. Packed atom
positions form the third dimension inside each region of both searches.
The exhaustive `zetesis-ferraris` checker provides a small-instance
reference for the region search in `zetesis-stable`.

Before the candidate walk starts, arithmetic preparation can refute its
root or fix atoms through proved units. Other atoms start open. Inside a
region, a sure reading guarantees that every interpretation in the region
satisfies the node, and a never reading guarantees that none does. A held
atom is sure and a cut atom is never. Narrowing requires each asserted root
to hold and propagates knowledge between nodes and their operands. For
example, a conjunction required to hold requires both operands. If a
disjunction must hold and one operand fails, the other must hold. These
steps repeat until the readings stop changing or a contradiction refutes
the region.

If an atom has no possible producer, narrowing cuts it. If it is already
held, the region is refuted. A held atom with one possible producer
requires that producer's body to hold. The `zetesis-ferraris` crate
applies these rules to a fixed point over the theory's DAG
(`FormulaBounds.read_sound`, `never_root_refutes`, `known_sound`,
`unsupported_cut`). A surviving region with open atoms splits on the atom
preferred by the ranking below, held branch first. At a fully decided
leaf, each asserted root is sure or never. A never root refutes the leaf,
so this region walk proposes only classical models of the original theory
that satisfy its active candidate restrictions
(`FormulaBounds.decided_leaf_models`).

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

Some producers cannot support their head at all. A body requires the atoms
that are its conjuncts, and an atom is derived only through `a` when it has
producers and every one of them requires `a`. A producer whose one head is `a`
is circular when its body requires `a` itself or an atom derived only through
`a`, as `a ← b` is beside `b ← a` when nothing else produces `b`. Every atom of
an answer set has a supporting producer that is not circular
(`ExternalSupport.answer_set_noncircular_support`), so the support scan passes
over circular producers (`noncircular_unsupported_cut`,
`noncircular_sole_rule_forces`, `noncircular_sole_choice_forces`). Such an
alias pair then needs no closure of possible founded support: when the other
producers of `a` fail, the scan cuts `a` and then `b`, and holding `b` forces
the one remaining producer of `a`. The marks are made once with the producer
inventory. The pass reads each body's own operands once for every producer
sharing the body and, for each head and each atom its body requires, that
atom's producers up to the first that does not require the head. On the 288
programs measured for this change that is between 0.4 and 1.4 times the
extraction's own visits, with a median of 0.8. It is not proportional to
them: the extraction visits a conjunction once, whatever its width, and one
large body shared by many rules is read once for each of them. The pass may
therefore spend at most four times the extraction's visits, and never more
than the extraction's work ceiling leaves. When it would exceed either it
marks nothing, and narrowing proceeds as it did before producers were
marked. Width alone can exceed the share: in a ring where each atom has one
rule requiring its own head after the next atoms of the ring, bodies of
seven atoms are marked and bodies of eight are not. The extraction's
reported work stays within its ceiling, so the walk's opening, which charges
that work to the budget the ceiling came from, is never refused that charge.
Under a finite work budget the marking's visits are otherwise spent like any
others: the rest of the opening has that much less, and the standalone
membership check, which extracts under a fixed ceiling and charges its own
budget, can be refused the larger charge although its preparation reads no
mark. Only producers with one head are marked, and only a body's own atom
operands are read, so the restriction is used less often than the law
allows. The roots themselves are unchanged and still propagate as
implications.

When some positive cycle of the complete producer inventory survives without
the circular producers, narrowing also restricts candidates by possible
founded support. A dependency component whose every cycle passes through a
circular producer is left to the support scan; a component with a surviving
cycle is closed whole and reads every producer, marked ones included. Its
closure then differs from one without marks only in its seeds, which include
the atoms of the components left to the scan. Finding the surviving cycles
takes a second component search when any producer is marked, over the
supporting dependencies and then over all of them, and both are charged; an
inventory without marks is searched once. The defining whole relaxation
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

A worker's tier of learned rows is bounded by its owner's limits: by default 64
rows, 256 founded capture attempts and 131,072 capture reads over the tier's
life, and per capture at most 4,096 reads, 65,536 workspace bytes, 16 signed
conditions and 64 outputs. Identical conditions can share their output union,
and a stronger newly certified row can replace older rows requiring more
conditions and giving fewer outputs. Unions retain the limit of 64 distinct
outputs. Merely overlapping conditions are not combined. Comparisons, packing
and bounded compaction are admitted before any old row is removed; an
interrupted attempt preserves the old certificates. Unrelated rows retain their
order, and an evicted row's position takes the new row. By default the founded
floor equals the tier's capacity, so a tier full of founded rows stops capturing
as before. Matching and capture are counted in `support_explanations.work`,
separately from `founded_work`, and both contribute to total reads.
Consolidation changes the physical representation of established consequences,
not their reduct meaning.

Up to 64 rows, each matching step tests every row. Above that, a step reads an
occurrence index of every row's condition and output atoms, and tests only the
rows that mention an atom decided since the previous step. Within one narrowing
knowledge only grows and a row's deduction reads only its own atoms, so a row
the index skips has the deduction the previous step already applied. A
narrowing's first step, whose inherited knowledge no earlier step of this tier
read, and the first step after the rows change test every row. The first indexed
step after a change rebuilds the index at one read per indexed occurrence; an
interrupted rebuild is never read.

Sharing rows is charged as it runs. Each store row a seeding passes over or
copies, and each tier row a delivery examines or delivers, costs reads of the
walk's budget, counted in `support_explanations.work`. The tier's sharing
allowance, 131,072 reads over its life by default, bounds the copies and their
comparisons; exhausting it ends that tier's sharing, never the walk. A tier that
read the whole store before reads only the rows retained there since. One lock
serializes the store, held by seeding and delivery and never by a narrowing. The
work quota, a cancellation or an allocator refusal during seeding or delivery
stops the walk as any read does: a walk whose final delivery exceeds the work
ceiling ends incomplete and claims no exhaustion. A tier delivers only under its
own class, the theory instance whose carrier learned its rows. Tier and store
capacities grow with the candidate memory allowance: one tier row per MiB of it,
between 64 and 4,096 rows, and a store four times a tier, so the library's
default allowance keeps today's 64 rows. Under sharing, work counts depend on the
schedule, since which worker learns a row first affects the others; answer sets
do not.

After a complete premise match, packed output words are filtered against current
held and cut masks before decoding atom IDs: a required absence is visited when
its atom is held or not yet cut, a required presence when its atom is cut or not
yet held. Outputs already satisfied need no event; a contradicted output must
still refute the region. Ascending event order is preserved, an absence before a
presence of the same atom. The cursor lasts only for that traversal and stores
no knowledge for another region. Work includes loaded words and emitted outputs.

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

A conjunction can also be excluded when its necessary contributions would
reach or exceed a unit row's exclusive upper bound. Preparation joins conjunction
operands with counted conditions, including aliases established by original
root implications. It counts distinct conditions once and adds only those not
already held to the processed held count. Pending truth can weaken that bound;
its processed event schedules another check. The projection is immutable,
stores only intersections with at least two conditions, and is unused during
frozen-reduct narrowing. This excludes impossible alternatives without changing
the membership check or retaining another per-region truth cache.

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
operation `zetesis_algebra::words::union_shifted_until_hit` checks translated bits
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

Native extremum cuts use their shared ordered guard column to locate the
affected prefix or suffix before sending consequences. Binary boundary lookup
and visited targets are charged; unrelated guards are skipped. Equal cuts,
frozen-truth checks and target order retain their existing behavior.

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

Before preparing that order, the same original-candidate owner recognizes
checked affine equations and cardinality bounds. Clause groups and complete
normal definitions supply parity equations over the original atom coordinates.
Uniform positive thresholds and eligible constraints supply guarded count
bounds. Recognition proposes each row; a separate source read authenticates it
against the exact theory and complete producer inventory.

For a bounded definition, signed atom columns are intersected within each body
and united across its producers. XOR with the head column gives its forbidden
patterns. These packed reductions retain every source check. The affine-hull
reduction stops adding differences at full rank, while counting the remaining
patterns to establish exactness.

Nested conjunctions retain every leaf occurrence. In the declared clause
contexts, an implication with the exact canonical-true antecedent is read through
to its signed atom. An asserted conjunction may supply both lower and upper
threshold bounds, including under original-world double negation; every child
must qualify before the family is retained. These are checked source views, not
rewrites of the original formula or its reduct.

Clause recognition also uses independently checked positive facts. Selected
original rules with atomic heads and positive conjunctive bodies feed the
shared positive activation kernel. Its proposed facts carry ordered original
rule witnesses; a separate replay starts empty and checks every premise before
accepting a fresh head. Unseeded cycles cannot justify themselves, and unknown
atoms are not assumed false. Recognition first checks the original clause's
distinct atoms and eight-atom limit, then removes known positive leaves or
discards a body falsified by a known negative leaf. The equation checker rereads
that original clause using the same checked fact owner. Complete definitions
and guarded counts retain their own source conditions.

Root parity elimination uses packed row XOR. The counting preparation checks
that proposed lower and upper families are disjoint covers of the same atoms,
then compares their totals. A guarded bound contributes its unconditional
fallback while its guard remains open. A proved root contradiction starts no
candidate walk; otherwise one-atom parity rows seed the root. The original
theory and frozen-reduct membership checks are unchanged. These preparations
run once for each original candidate owner, before serial, parallel or device
candidate scheduling; they are host preparation, not device kernels.

Both systems share an optional allowance of 100,000,000 logical word operations
and the inherited preparation-byte allowance. Exhausting that allowance during
parity or cardinality preparation discards those preparations and keeps ordinary
candidate generation. Cancellation, global work refusal, allocation and invalid
source reads remain typed failures. Only a completed preparation publishes rows
or units.

An additional original-root image closure combines checked mandatory groups
with independently read forbidden pairs. If every remaining member of a
mandatory group excludes an atom, that atom can be cut; an empty mandatory group
refutes the root. The closure keeps original atom coordinates and works on a
private root copy. Only a completed result can replace the prior seed or publish
a refutation. Its selection, pair index, copied root and closure share the same
remaining work and byte allowances; they do not enter frozen-reduct membership.
An optional refusal of this additional transaction retains the earlier complete
preparation. Other typed failures still propagate. This retention choice is
internal and is not exposed as an execution option. The
[root-image observations](../rust/outcomes.md#views-and-failures) distinguish
its private work from publication.

The default candidate walk uses the prepared root and skips further parity and
cardinality inspections of candidate regions, even when preparation found a
nonempty basis or a checked pair of counting families.

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
The scalar `zetesis_algebra::walk::Walk` covers candidate and proper-subset
regions with the caller's narrowing. With several workers, `zetesis-stable`'s
parallel regions walk the same tree: the walk begins on the calling thread,
and the workers it then starts each own a deque of regions, from which idle
workers steal. That schedule and the GPU candidate producers share the queue
and termination protocol in `zetesis_algebra::walk::frontier::Frontier`.
Their original-theory and frozen-reduct narrowing remain distinct.

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

The query walk has subjects of its own. After its root's first closure the
candidate's inner subjects are prepared from that closed root: the guards
whose holding reaches the candidate are cut as units; each clause group's
patterns that a clause does not already forbid are probed, in at most two
rounds, by narrowing a copy of the closed root with the guard and the pattern
decided (a group of one clause cutting its whole support only in the second), and the allowed patterns' affine hull and least and greatest counts, with the cliques of pairwise exclusions, at most one atom held, the cliques of covering pairs, at most one atom cut,
and the trivial bounds completing a partition, supply rows and bounds to the
parity and cardinality systems for this query. From then on every region of
the query, inline or served, narrows through the one joint driver with the frozen
reading eager and these systems late, each preparation authenticated to its
candidate at every use and dropped with the query. The receipts' `preparation`
object (see the outcomes reference) reports what was prepared and what it cost.

Because the regions share nothing, several workers can walk the tree at
once (`--threads`, the host's available parallelism by default), started once
the walk has run on one thread, in wall time since it began, for longer than
their start costs, each deciding the leaves it reaches. The family is exact at
any worker count, each answer arriving once, by the partition law; the order in
which answers arrive is the scalar walk's until the workers start and the
schedule's after, is not promised to repeat between runs, and is not a property
of the result. Verification rests on the laws and on the
oracle comparison of answer sets as sets, not on order or determinism.

A worker decides each leaf it reaches, so a candidate whose subset query is
wide would hold one worker while the others idle once the candidate frontier is
thin, as on a 2QBF instance with few existential and many universal
assignments. Such a query is divided. Once it has run, since its first
narrowing, for the cost of engaging one worker while another waits idle
(`QueryDivision::WhenIdle`, the default), it stops before a narrowing and
publishes its pending regions, each with its frozen-narrowing knowledge, to its
worker's deque of published regions; the regions it visited and the ones it
published partition its tree. Any worker may then serve them, narrowing on its
own reduct workspace and charging its own lease on the walk's allowance. A
worker serves its own published regions before its candidate regions, and with
both deques empty steals a peer's oldest published region before a peer's
oldest candidate region. The query keeps one count of its live regions, raised
before a split's children are visible. A validated countermodel refutes the
candidate at once; the last region resolved with no countermodel and no stop
proves it stable, and its server sends it. The leaf stays unresolved in the
walk until then, so the walk cannot end beside a query in progress, and a stop
or a region whose check stopped never becomes a stable verdict
(`InnerQuery.exhausted_walk_decides`). The coordinator publishes only once its
engagement deadline has passed, and then engages the workers to serve the
query. A refused allocation keeps the query inline.

The allocation refusal that keeps a query inline belongs to the initial handoff:
its pending entries have not yet been surrendered. A later served-query split
retains its parent until child capacity is admitted. If that reservation fails,
the server records the query stop and the outer typed stop before abandoning the
returned owner. Retirement after that stop is cleanup, not evidence of a stable
candidate. Candidate workers and slate producers follow the same owned failure
boundary; queued and in-hand entries preserve the unresolved partition until
terminal abandonment. A failed return to a deque also returns its entry to the
caller, including when a thief's home has no capacity or a shared home has gained
other work. These paths preserve the original allocation error and claim no
exhaustive result.

Answer sets are the same under every division. A query that ends stable narrows
each region once, wherever it is divided; a refuted query, divided, may narrow
regions beside the subtree its witness lies in until a server finds a witness,
so its region count depends on the schedule. `SolveConfig::query_division`
selects `QueryDivision::Never` to keep every query on the worker that reached
its candidate, for measurement; the command has no flag for it. The workers'
statistics report the division, the queries published, the published regions
served and stolen, the workers that served any, and the most one served.

Candidate production and membership need not run on the same executor.
`StableModels::with_region_producers` uses the same original-theory readings
and disjoint splits, filling each slate (the candidates checked and committed
together, below) in one bounded Rayon run. A worker returns a classical leaf;
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

A producer holds a region only in one of the slate's hands, as many as the
slate admits candidates and never more than there are producers. A split keeps
the hand for the child the producer continues and publishes the other child,
where an idle producer can steal it; a refutation or an emission keeps it only
when the producer finds another region at once, and otherwise releases it. A
step reserves an output slot only when it reaches a leaf, so the slate is
bounded even when producers finish together, and its end does not leave the
search for its last candidates to the producers that have found none. Once
every output slot is reserved the output is spoken for. A producer reads it
after reserving its hand, and once it has read it so it takes no region and
begins no step; a producer that has not yet read it holds one of the slate's
other hands, and its step ends as a step under way does. A leaf that finds the
output spoken for returns its region, unresolved, for a later slate.
The producers keep one deque each, retained between slates, and share the
region walker's frontier protocols: the count of unresolved regions, split
publication and the idle back-off. A producer steals only with an empty deque,
so its deque holds at most one sibling per atom split along its current path.
That order suits an enumeration, which visits every region whatever the order.
An optimizing search that generates candidate bounds tightens the bound as it commits answers, so its
producers instead share one deque and each takes the region any of them
published most recently. Production then stays where the latest splits are,
and older, shallower regions wait for the tighter bounds that refute them near
the root; producers kept apart would each descend a large subtree under the
bound in force when they took it. The shared deque gives up the per-deque bound
on retained regions, which only the tree then bounds, as with the single stack
the producers used before. Each holds one work lease for the slate. A cancellation or deadline observed
once the slate is full is left to the next slate's first poll. A stopped
validation retains its admitted prefix and reports the unfinished suffix
explicitly. Every producer joins and settles its work before membership begins.
Original validation has its own per-candidate work allowance. On a device
route, an enumeration produces its next slate while the device checks the
current slate's last submission, within one call; optimizing searches and the
CPU routes do not overlap production with device execution. The slate's own
producers still join and settle before its device check begins, and the next
slate is submitted only after the current one commits. Pending-byte limits cover
pending/result storage and original-validation scratch, not the entire region
frontier, thread stacks or process memory.

The generic batch-checker API independently validates original satisfaction
using one `EvaluationWorkspace` per owner. Each candidate receives a full
original-formula evaluation; only allocated capacity survives between candidates. The pending-byte allowance
includes the workspace header and actual capacity. A smaller allowance can drop
optional scratch without invalidating previously checked pending candidates.
Retrying their external checker does not repeat original validation. Separate
receipts report validation work and retained bytes.

Device execution uses the explicit joint-check protocol. A successful
`CheckedBatch` binds decoded results to the exact theory and candidate slice;
an overlapped check's `CheckedSlate` binds them to the theory, the slate's
identity, and the submission's offset and count;
all non-`NotModel` results establish original satisfaction, including residuals.
The coordinator consumes these receipts in order and commits only after the
entire pending batch succeeds. A failed device call retains pending candidates;
retrying through the generic API first performs independent original validation.
`NotModel` is a producer invariant failure and cannot become exhaustion or a
partially published batch. The trusted callback contract and actual device
execution remain separate from mechanically proved correspondence.

The executor and its producers' workspaces persist between slates. Each
workspace keeps its narrowing worklists and, when needed, one checker for the
immutable source. Workspaces are used exclusively, may move between executor
threads and number at most the requested workers. The retained checker borrows one stable owner;
stopping or completing production drops the checkers before that owner.
Candidate truth and support knowledge still travel with each region, so reuse
does not transfer deductions between unrelated candidates. The streamed
constraints' checkers keep their own lifetime, separate from formula evaluation.

The original CPU `with_region_workers` operation remains useful when each
worker should perform both operations. The two schedules share the original
formula readings, support conditions, candidate partitions and reduct semantics.
The [batch API](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-stable/src/ferraris/batch.rs)
owns proposal validation and publication; the
[producer](https://github.com/GregoryGelfond/zetesis/blob/main/crates/zetesis-stable/src/ferraris/region_proposals.rs)
owns the bounded parallel frontier. `Frontier` and `BatchAccounting` state the
coverage and completion laws; concrete synchronization remains a refinement
obligation.

The region search became the default by
[measurement](https://github.com/GregoryGelfond/zetesis/blob/main/docs/book/reference/observations/README.md#the-regions-default-and-one-worker-count)
beside a classical search over a clause form on the same cells, with the host's
workers on the tree. That clause search has since been removed. Read as regions
it walked the same tree: a search node held some atoms in and some out,
propagation added the atoms every classical model agrees on under those
decisions, a conflict closed the node, and a complete assignment was a leaf the
reduct decided (`FormulaRegions.classical_consequence_forces`,
`no_model_refutes`). Neither search decided membership by anything but the
reduct, and neither kept learned clauses.

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
open region (`Bounds.undecided` in the Lean), `Bounds.lower_constraint_refutes` and
`Bounds.conflicting_atom_refutes` for the refutations, and the coverage
tree of `Search.lean`, whose `split` node with `Region.split_partition` and
`Region.split_disjoint` makes the regions a partition of their parent and
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
leaves a proper-subset model of the reduct. The removed clause search restricted
its outer proposals by that condition; independent producers can still support
several atoms in one disjunctive head. The region search's support cut reads the
same producers inside its narrowing. The original theory and frozen reduct are
unchanged.

The [gate restriction laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/GateRestrictions.lean)
and [disjunctive support laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/DisjunctiveSupport.lean)
state these necessary conditions. Source binding coverage and the executable
certificate constructors retain separate refinement obligations.

For a complete positive atomic-head theory, `PositivePlan` computes the least
consequences once and checks every original constraint. CPU region modes then
propose only that interpretation, or none when a constraint fails. Accumulated
candidate restrictions and the streamed constraints still apply to the
singleton; the ordinary membership and publication checks retain the original
theory owner.
No worker needs to explore candidate regions or allocate a reduct query for this
case. Larger classical models need not be individually refuted: the certificate
already rules them out as answer sets. An unsupported or resource-refused plan
keeps general solving available; an interrupted attempt never establishes
exhaustion. This CPU policy does not change explicit device checking.

The removed clause search also kept an exact projection history of the
candidates it proposed, packed its clauses in one literal arena, and replaced a
ternary clause's watch by position. The region search needs no such history,
since it visits a leaf once. The
[packed literal laws](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/PackedQueryLiterals.lean)
and the [ternary law](../lean/correspondence.md) remain as statements; no
current route implements them.

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
can still produce a typed capacity refusal. Pre-dispatch admission uses this
allowance: an exact allowance is admitted and one less is refused. The complete
and dense routes' receipts charge their fixed allowance; an update's receipt
charges the work it performed, never more than was reserved (below). Historical
work counts need their source identity when compared. These counts do not
measure latency or change membership.

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

Dense reuse still visits every node. Successive batches of one theory and count
can instead *update*: each candidate slot retains the candidate it last
evaluated, that evaluation's enabled-producer count per head, and its sets of
false root positions and unsupported present atoms, and re-evaluates only the
nodes a new candidate's change reaches. A readers index, built once per theory
at the first batch holding reuse authority and appended to the producer
storage, lists for each node the connectives and numeric-row owners reading it,
the heads of the producers whose body it is and the root positions holding it,
and for each atom its atom nodes. An update first counts the slot's changed
atoms; beyond `max(atoms / 8, 64)` the slot takes dense reuse instead. Otherwise
it queues the changed atoms' nodes and visits the dependency levels in order.
Each level takes exactly its own queued bits, so readers queued for later levels
in a shared word survive, evaluates those nodes from final lower-level truth,
and for each changed value queues its readers, adjusts its producers' heads'
counts and flips its root positions. A numeric row's owner re-reduces the row
and writes every live guard, the followers included, which share its level
because all of them read the row's conditions. Afterwards the touched atoms'
unsupported bits are settled. The witnesses are the least false root position,
or else the least unsupported present atom, exactly as the dense scans find
them. Every node then holds its truth under the new candidate: the node-table
rule of `TightEvaluation.updated_values_exact`, applied in level order.

The route engages only where dense evaluation is wide enough to repay it.
Dense evaluation of one candidate takes `I(W) = Σ ⌈n_l / W⌉` strided iterations
over the level widths `n_l` at the dispatched workgroup size `W`, and
synchronizes the workgroup once per level and three more times. An update keeps
the per-level synchronization, adds five more, eight when it summarizes, and at
each level evaluates each queued word's taken nodes, maintaining their readers'
queue bits, counts and root sets through storage atomics, in one invocation per
word. A level therefore costs an update several dense strides even when little
changed. A schedule engages the route when `I(W)` is at least
`GpuTightOracle::UPDATE_ENGAGEMENT_STRIDES`, thirty-two, times its level count.
Like the fitted size, the decision is structural, made once per resident theory
and dispatched size, and on engaging schedules the change threshold still
selects each slot's route. The factor is an execution choice and changes no
verdict. Calibrated by measurement on an Apple M4 Pro GPU under Metal, it covers
the change patterns measured there, their per-level chains included: scattered
changes, and clustered changes of up to 64 atoms with one reader per input. The
runtime guard below bounds the patterns beyond them, such as a change reaching
many nodes of one queued word. The factor still decides whether a theory builds
the index at all, sparing narrow theories the index, the state, the seeding
batches and every losing probe, and it covers the update's extra
synchronization, which the guard cannot see. Declining keeps the dense routes,
so a factor set high forgoes gains on the schedules it declines, and one set low
slows those it engages.

On an engaged schedule a runtime guard bounds the loss of changes whose critical
path, rather than their size, makes an update dearer than dense evaluation. It
compares two paths counted by one cost table, which weighs the same evaluation
alike on both routes: a node 1, each operand or condition it reads 1 (a wide
conjunction or disjunction only those it reads before an absorbing value ends
it), each readers entry an update visits 1, and each item of a scan 1, or 2
where the scan reads two words per candidate word. Each update record carries
the slot's path: in each phase after its summary, the costliest invocation of
its workgroup, summed, with each invocation's share of the uniform scans and of
the record. A summary after a batch that seeded every slot, such as the batch
after the index is built or the guard's probe below, is a cost once per seeding,
counted in the work but not in the path. A summary after the slot took dense
reuse beyond the change threshold recurs with the changes, so its phases count
in the path too, and the guard weighs them. The host computes dense reuse's
path for one candidate, its busiest invocation over each level and each phase
of support construction, once per resident theory and dispatched size in the
pass that computes `I(W)`. It is a lower bound: where dense reuse's cost
depends on the candidate, a wide gate's reduction or a numeric row skipped when
none of its conditions changed, the path takes the least. After a batch decodes and its reuse authority is
re-established, the guard judges it if any slot updated: when the batch's
slowest update path exceeds the dense path, the route is declined. A batch's
slots run concurrently, so a latency-bound batch waits for its slowest slot;
judging the slowest keeps the loss bounded whether the batch holds the device
or oversubscribes it, at the cost of declining a skewed batch whose sum would
have won.

The guard is engaged or declined. A losing judgment declines the route for the
next batch, and each later one for twice as many batches as the last, up to 32;
the wait never shrinks while the theory stays resident. While declined, every
decoded batch counts the wait down and none is judged. A batch the guard
declines runs the dense twin kernel, as a theory declined for its residency
does, with its state allocated and accounted but unbound, so its count, prefix
and reuse authority are an admitted batch's while every charge is the stateless
one; it reports `GuardDeclined`. Once the wait ends, the next batch seeds the
state, since every record written meanwhile says unseeded, and the batch after
it updates and is judged. A failed or cancelled batch is never judged and
consumes no wait. If every judged batch loses, at most one batch in 34 does so
once the waits reach 32. The guard reads receipts, never a verdict or a
witness, and changes which route evaluates a batch, never a verdict. The
comparison sees operations, not their time: an update's storage atomics and
indirect reads can cost more than dense evaluation's strided reads, invocations
updating one word serialize, and neither route's synchronization is counted.
Those errors vary with the device and the change, so how closely the guard
follows the faster route is a measurement, not a consequence of the comparison.

A narrower schedule declines the route with the typed reason `NarrowSchedule`
from its theory's first batch on. It builds no index and holds no state, and its
routes charge their stateless allowance and synchronize as they did before the
route existed. Which kernel a batch runs follows whether its update route is
allowed: the full kernel exactly when it is, and otherwise the kernel's dense
twin. The twin therefore runs a theory declined for its whole residency, by a
narrow or serial schedule or by refused index bounds; an engaging theory's
batches before its index is built; and a batch whose route a limit or the guard
declines. The twin is an entry point that evaluates complete or dense truth as
the full kernel's complete and dense routes do, and whose call graph never
names the route. Its pipeline therefore binds no state and compiles none of the
route's phases, counters or decisions. Each variant is compiled at its first
need, so a theory that engages the route compiles both. A batch that admitted
the state keeps it allocated and accounted while the twin leaves it unbound.
What the route still costs a declined schedule is its larger parameter block
and result records, 64 rather than 32 bytes per batch and 36 rather than 24 per
candidate. Admission of a scheduled layout also reserves the owner column and
the per-head producer counters before the level widths are known; a narrow
schedule releases both once packed, and its batch accounts neither. An exact
call whose cold batch fits the scheduled layout only without those
reservations, within `8 · live numeric rows + 4 · atoms` bytes of its
allowance, keeps the theory on the serial schedule while it is resident; prefix
calls, which admit their cold layout at one candidate, are unaffected.

Each slot's state counts in the batch's byte allowance once admitted. A prefix
call admits it only when the state-inclusive layout still admits
`min_update_candidates` candidates, which the solver sets to its batch size, the
slates' floor; an exact call admits it only when its whole count fits. Otherwise
the dense routes run, and a state held from an earlier batch is released while
truth and result authority remain; the next batch admitting the state allocates
it zeroed and seeds it. With the floor, the state never splits a fixed slate,
and adaptive slates settle at or below the prefix the state admits, since their
growth stops at the smallest prefix the device has admitted, the batch building
the index included; without it, a shorter prefix would split every slate into
submissions of alternating counts, which hold no reuse authority. The tight
check has its own byte allowance for this reason: a sixteenth of the memory
allowance under the ordinary `Resources` policy, where every other device batch
has a thirty-second, so the state reaches the floor under smaller allowances.
On a theory of about 790,000 nodes and 262,000 atoms, the default allowance
admits it on hosts with about 9.3 GiB of memory rather than about 18.7 GiB.
Authority
needs successive batches of one count, so neither reuse route runs while
adaptive slates are still growing. The index stays resident with the theory and
remains accounted, like a retained schedule. The route is allowed only when its
derived worst case, and dense reuse after a change count, fit the work
allowance. While it is allowed, the complete and dense routes also retain the
evaluated candidate, `words + 1` operations. Each result record carries its
route with whether the slot's state was seeded, the nodes an update evaluated,
and an update's path; an update's work receipt is what it performed, at least a
fixed floor of `5 * words + ceil(nodes / 32)` word operations, one queued-set
scan per level and the record. Batch statistics report the slots per route, the
route's availability, with a typed reason when it is unavailable, and the
guard's two paths. The route's speed is a measurement, not a consequence of
these bounds: on narrow changes it replaces most node evaluations by word scans,
and on dense changes it falls back to dense reuse.

Only complete decoding, healthy context completion and the final cancellation
check authorize reuse. Dispatch consumes that authority before queue writes;
any failure, changed theory/count or explicit clear invalidates it. Without
valid authority, the same encoder clears the prior result hints before computing,
including each slot's seeded record, so no update follows lost authority.
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

The general GPU route also reports `gpu_residual_handovers`. Its latest attempt
names the slate sequence and the attempt ordinal. On retry the slate keeps its
sequence. A stop retains decoded residuals and work already performed. Only
publication increments the committed-slate count. A slot settled by a CPU
certificate contributes a residual but no regional search.

When recording space permits, the coordinator joins the GPU reason and number
of open atoms to the CPU receipt at the same original slot. It totals work and
outcomes by reason. A slate's width is the maximum number of visited regions
at a single depth, summed across its queries at that depth. Host time sums the
individual query intervals, which may overlap. Unavailable measurements appear
as null.

The optional `gpu_residual_handovers.samples` report describes two populations
across the run. Use `worlds` to ask how difficult the residual queries were.
Use `batches` to ask how much work was handed over at each dispatch. Every
successful batch enters once, including one with zero residuals. A failed
attempt contributes to the attempt's work receipt. Its retry enters the batch
population upon successful commit.

While the session is live, the report gives population sizes and storage use.
At settlement, `groups` gives the combined view and a view for each device
stopping reason when recording space permits. Each measurement shows how many
members were eligible, measured and retained. Its median uses the integer
midpoint for an even sample. Its 90th percentile uses nearest rank. Ranks
are marked `exact`, `estimated` or `unavailable`. Disabled timing stays
unavailable. A stopped run describes the committed prefix of that run.

The collector aims for 65,536 worlds and 4,096 batches. Its separate byte
allowance is one eighth of the general GPU batch allowance. Lowering memory
can reduce either sample or omit the report. The arrays belong to the
session. Reading a snapshot never advances the samples. The
[benchmark evidence campaign](../reference/benchmarking.md#pool-gpu-residual-observations)
can request the actual records at settlement and combine separately qualified
instances at their original population weights.

For a separately requested benchmark capture, a Rust caller can offer an
empty, privately held file with `Session::request_residual_evidence` and
retrieve its one-shot receipt with `Session::finish_residual_evidence` after
settlement. The host streams the two retained populations before selecting
terminal ranks or releasing their storage. Only a sealed artifact carries the
slot identity, population sizes, seed and complete-family or prefix status.
An unavailable collector, insufficient file allowance or failed write leaves
the ordinary solver outcome available. The benchmark's child request uses a
private directory, verifies the matching answer document, and keeps the raw
records outside ordinary CLI output.

The optional slot journal uses idle CPU completion scratch during decoding.
Before the CPU starts, the coordinator retains the journal only when the
remaining GPU batch allowance covers it beside the admitted owners. Otherwise
it releases the journal. Exact search proceeds with its original scratch limit.

With several requested workers, the region proposer produces a bounded batch
in parallel and joins before its own slate's device check starts. On an
enumeration, the next slate's production overlaps the current slate's last
submission; nothing else overlaps. A control stop during that production leaves
the slate under check pending and undelivered, claims nothing for it, and lets
the device observe its submission's completion. On a GPU formula session, the
next slate's production and the current slate's residual completion draw on
the stream's one cumulative search budget, production first: a work or decision
limit that the production reaches, or brings within reach of that completion,
leaves the slate under check pending and undelivered in the same way, while a
slate the device decides wholly draws nothing at its commit and is delivered
before the limit is reported. Membership, scoring and objective feedback keep
the same original-theory and completed-batch boundaries. Failed device execution
never triggers a hidden CPU replacement.

A slate is the candidates the stream proposes together for one membership
check and commit; the device checks a slate in one or more submissions. GPU
formula sessions size their slates adaptively by default
(`GpuBatching::Adaptive` in `SolveConfig`). Slates start at `batch_size`. The
next slate is sized when it is selected, after the previous slate committed
completely and its answers were scored. On an enumeration on a device route,
the next slate is selected while the previous one is checked, from that slate's
proposal and its submissions' admitted prefixes; an enumeration declares no
bounds, so scoring cannot move its size. It returns to `batch_size` when those
answers tightened the objective bound, doubles after a filled slate that left
the bound unchanged, and otherwise keeps its size. The session's last selection
follows its last non-empty slate and precedes the empty proposal that
establishes exhaustion; on an enumeration, that proposal runs during the last
slate's check, and exhaustion is established after its commit. Growth stops at the smallest device prefix observed,
never below `batch_size`, and the pending-memory allowance bounds every slate.
Under either policy a device submission takes as much of a slate as the device
admits. Failed checks retain the same pending candidates and size. Larger
slates mean fewer slates and submissions, but release their answers only after
the whole slate is checked and, on an enumeration, after the next slate is
produced, and are produced under the bound as it stood when
they started. `GpuBatching::Fixed` keeps every slate at `batch_size` for
controlled comparisons. Adaptive sizing is not a performance or allocation
guarantee, and it never changes answer sets, optima or ties.

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

Each kernel strides every level, and every dense node, atom, root and word loop,
by its workgroup size, the invocations cooperating on one candidate. By default
the size fits the retained schedule. With `I(W) = Σ ⌈n_l / W⌉` the strided depth
over the level widths `n_l` (each level's node count), the kernel dispatches,
among the granted sizes 64, 128 and 256 whose invocation slots `W·I(W)` stay
within twice those of 64, the smallest size attaining the least depth. A serial
schedule, or one whose levels each hold at most 64 nodes, keeps 64. A widest
level of 65 to 128 nodes selects 128, and 256 needs a wider level and
`I(256) ≤ I(64)/2`. The selection depends on the level widths and the granted
limits only, not on the adapter's subgroup size. Where a batch leaves the device
latency-bound, the dense level pass takes time following the strided depth;
where a batch oversubscribes the device it follows the invocation slots, which
the bound keeps within twice the 64-invocation time. The bound covers that pass
only: the tight kernel's serial bypass for a tight work allowance occupies the
whole workgroup with one invocation, up to four times the 64-invocation cost,
and the opt-in active-word routes are not modelled.

Creating a kernel validates its shader. Each fitted size is compiled by the
first check that dispatches it, before that check's dispatch, so a solve
compiles only the size it uses. A failure to compile fails that check and
invalidates the context, as every compilation failure does; no theory stays
resident, because only a freshly prepared theory can need a size first and its
cold preparation released the previous one. `GpuKernelOptions` can fix one size, which is then
compiled when the kernel is created. The batch statistics report the size dispatched. A
size changes which invocation evaluates which node, never a node's value. Under
frozen-reduct propagation sibling gates narrow shared domains through atomics,
so the number of sweeps, and whether a candidate reaches a sweep or work limit,
can differ between sizes; such a residual is completed exactly on the host.
Away from those limits a candidate's verdict class agrees across sizes where
every transfer is order-preserving, which the contract tests establish for
binary gates; for native groups, thresholds and extremum cuts it is a stated
[implementation obligation](../lean/correspondence.md#ownership-and-execution-correspondence)
that the physical tests check but do not prove. Work allowances do not depend on
the size.

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
A residual returns the held and cut masks for the semantic atoms of one
candidate. The CPU starts from that region with fresh frozen-reduct knowledge.
It performs the first narrowing even if the masks leave only one possible
interpretation. A residual classifies every atom and writes two mask words per
group of 32 atoms. A terminal result only writes the zero masks.

Mandatory device setup reserves `2*nodes + edges + atoms + roots + levels`
for the formula and `atoms + 2*ceil(atoms/32)` for its result. Pure chains
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
| Independent formula membership | Scalar CPU or owned completion workers |
| Hybrid source constraints | Host checks before original-answer acceptance or scoring |
| Ordinary GPU formula membership | Complete tight support when certified; otherwise device propagation plus exact native CPU completion of residuals |
| Native aggregate reductions | Bounded reusable CPU/device primitive; availability alone does not imply use by every solve |

wgpu supplies portable device access, including Metal on supported macOS
adapters. With default resources, device availability is discovered at runtime.
A command-line GPU solve begins wgpu's adapter enumeration on a background
thread once its configuration is validated and before it reads its sources, so
the adapter enumeration overlaps reading and admission, and on the formula
routes grounding as well. The solve's GPU primitive then selects over those
adapters with its own capability check, as over an enumeration of its own,
and requests its own device. Adapter choice and
errors are therefore those of selection at the moment the enumeration began,
which precedes construction by the time to read and admit the program and, on
the formula routes, to ground it, so the gap grows with the program; timestamp
requests are unchanged. An adapter added or removed in between is missed, or
fails its device request (`GpuDiscovery` documents the snapshot). A
solve that ends before constructing a primitive waits for the rest of the
adapter enumeration, and wgpu's loader can report on it; a CPU solve starts none.
Library entry points select per session unless their caller supplies such
resources. Requesting a backend is
not evidence that it ran; adapter identity, submitted work, completed work and
residual work are separate observations. GPU support does not mean the complete
solver is device-resident.

Automatic and lazy formula materialization can defer eligible terminal positive definitions and
reconstruct them after base membership; other formulas are materialized eagerly.
Explicit lazy execution can stream eligible constraints against answers of its
retained producer core, whose membership can use the selected CPU or GPU backend.
Formula device membership consumes the materialized theory; streamed constraints
and terminal reconstruction run on the host.

CPU is the default backend throughout the solve. Existing measurements do not
establish a device crossover for an automatic hardware policy; batch size alone
is insufficient evidence. An explicit GPU backend prepares its executor during
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
requests optional wgpu timestamp queries. Each formula or tight batch then reads
the previous batch's compute pass through its own submission and readback, after
the host has observed that batch complete, and a settling execution reads its
final batch with one resolve-only submission. Unsupported, invalid or equal
counter readings remain explicitly unavailable: equal readings do not establish
zero compute cost. Caller-supplied contexts retain
their granted features; statistics never replace them. Timing storage is accounted
separately and absent when disabled. Logical storage budgets are not process RSS or total device memory.
The [outcome contracts](../rust/outcomes.md) preserve these distinctions for
library consumers.

The [neuromorphic appendix](../appendices/neuromorphic.md) describes a proposed
mapping of the same semantic operations to Loihi 2 and SpiNNaker2. Its event
protocol and qualification steps are design obligations; these targets are not
implemented backends.
