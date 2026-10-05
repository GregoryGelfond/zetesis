# zetesis-ferraris

A native exact kernel for **finite propositional formula reducts**. The Horn
oracle in `zetesis-cpu` computes a least closure. A general reduct may have
incomparable minimal models; this kernel checks proper subsets instead.

The input is a bounded, immutable DAG of atoms, falsum, conjunction, disjunction,
and implication, plus asserted roots. Default negation is implication to falsum.
One pass computes the candidate's classical truth mask. A second transform
evaluates the frozen reduct in a proposed countermodel: every candidate-false
node becomes falsum. The checker rejects a classical nonmodel immediately,
otherwise enumerates proper subsets until it finds a countermodel or proves
minimality. Rejection includes the failed root or a proper-subset witness.

The exhaustive reference checker has work/subset bounds and shared CPU
cancellation/deadline control. It is not a production search strategy for large
disjunctive programs. The aggregate constructor below translates already finite
scalar aggregates into this same formula DAG. The crate does not ground source,
evaluate optimization objectives, or run on a GPU. Source translation and execution
refinements remain separate obligations.

`proofs/Zetesis/Ferraris.lean` establishes the denotational definition and its
equivalence to minimal reduct models. It does **not** prove this Rust code.
`proofs/Zetesis/FerrarisMask.lean` separately proves that a correct frozen truth
mask can replace explicit reduct construction during formula and theory
evaluation. Its hypothesis requires mask correctness; it does not verify DAG
indices, packed bits, work accounting, or the subset counter.
Tests compare the fused transform against an independently materialized tree
reduct for 3,600 two-atom theories, all four candidates, and all four tested
interpretations, including nested implication and non-Horn reducts. Boundary
tests cover finite identities, packed words, empty theories, exact limits,
cancellation, and deadlines.

## Reusing original satisfaction work

`EvaluationWorkspace::evaluate(&candidate, limits, control)` computes every
original node's truth, then checks asserted roots through the first failure.
Its `FormulaEvaluation` borrows the exact candidate and the workspace. Consumers
can inspect `node_truth`, `failed_root` and `is_model`; they cannot construct this
view from unverified bits or retain it while overwriting the workspace. A
successful evaluation can report a nonmodel. It does not establish an answer set.

The workspace retains capacity across candidates and independently admitted
theories, recomputing all values on every call. Work counts node and root visits;
bytes include its header and actual Boolean-vector capacity. A lowered byte
ceiling includes capacity retained by earlier calls. Cancellation, deadlines,
allocation and resource failures expose no truth view and retain their consumed
work and storage receipt. Borrowed inputs and allocator bookkeeping are excluded.
The `models` convenience operation uses this evaluator with a fresh workspace;
its theory-identity check and original work-limit behavior are preserved.

## Reusing a candidate's reduct

`FrozenReduct::new(&candidate, limits, control)` freezes the candidate's own
theory once. The value borrows that exact interpretation and owns its Boolean
truth mask. `is_satisfied_by(&tested, limits, control)` tests any interpretation
of the same theory instance, including interpretations outside the candidate.
Independent admission of identical syntax does not preserve instance identity.
No candidate words or original DAG are copied; `candidate()` and `theory()`
expose the bound subject. A frozen value is neither a model certificate nor an
answer set. Use the checked membership API for that conclusion.

Construction charges one evaluation per DAG node and retains one Boolean per
node. Each query allocates its own temporary Boolean workspace and charges one
evaluation per node, followed by root tests through the first failure. The value
is immutable and can be shared between independent callers. Construction and
each query have separate work budgets; these operations perform no subset search.
Cancellation, deadlines, foreign identity, allocation failure and exhausted
work remain explicit stops. A stopped query leaves the freeze unchanged.

The existing `models_reduct` wrapper still freezes anew on each call and uses one
combined work budget. Its identity checks and stop precedence are unchanged.
The exhaustive `check` operation already freezes once per candidate, so this API
does not speed up that checker. It gives library callers a way to avoid repeated
freezing when they separately test multiple interpretations against one reduct.
The `frozen_reduct` Criterion benchmark compares these caller patterns, including
and excluding initial freezing explicitly. It measures neither ordinary solving
nor grounding. No timing improvement is claimed without measurements.

Proptest adds generated DAGs with one to six atoms, up to 48 topological nodes,
shared subexpressions, deep chains, multiple roots, and permuted atom indices.
Each generated case checks classical truth, a frozen reduct under an arbitrary
tested interpretation, exhaustive stable membership, and returned countermodels
against an independently materialized tree. Test generation limits each root's
unrolled tree to 1,024 nodes; this protects the reference comparison from DAG
expansion without narrowing the admitted production language.

Proptest shrinks the raw instructions while preserving valid topological DAGs.
Failing seeds are persisted and replayed from
`tests/regressions/formula-dag.txt`; commit new entries after fixing their causes.
The tracked file is initially empty of failures because none has been observed.
Normal runs use Proptest's default case count; `PROPTEST_CASES` can increase it.
Focused deterministic tests also exercise shared DAG slots, sparse subset carries
across three machine words, exact work/subset limits, and all identity checks.

## Necessary mixed-head support

`support_restriction` recognizes every asserted root before constructing a
candidate-only supportedness theory. Ordinary positive disjunctive heads may
coexist with exact atomic choices `a or not a`, in either branch order and
optionally under an arbitrary original body. The choice reader is shared with
tight-plan extraction. Choices nested inside richer heads, cross-atom
alternatives and opaque asserted roots decline the complete specialization.
At least one ordinary syntactic disjunctive head is required; choices alone do
not trigger this optional construction.

An ordinary producer supports a true atom when its body is true and its other
distinct heads are false. An atomic choice contributes its body's permission
for its own atom; an unconditional choice contributes truth. Witnesses from
independent producers are combined. This is necessary support without a rank
premise, not a stability test or global exclusivity condition. Positive cycles
can remain and require the unchanged original reduct membership check.

Construction retains the existing finite formula/work limits and charged
failure prefix. No partial restriction escapes opaque-root, resource or control
refusal. The separate restriction still copies original DAG descriptors before
adding support nodes; repeated encoding of that copy remains a preparation cost.
See [support API](src/support.rs) and [complete small-family controls](tests/integration/candidate_support.rs).

## Narrowing regions by the theory's readings

A `Region` (`zetesis_cpu::regions`) holds some atoms in every candidate, cuts
some from every candidate and leaves the rest open; here it decides over the
theory's atoms. Under a region every node of the DAG has
two readings: *sure* when every candidate of the region satisfies it and
*impossible* when none does. A held atom is sure, a cut atom impossible, and
the connectives combine the readings as the closure route's definite and
possible gates do; the knowledge the narrowing closes means them, a node
known to hold being sure and one known to fail impossible.
`Narrower::narrow_known` closes the knowledge to a fixed point: every root
and every decided atom is known, a node learns
from its operands and teaches its operands what its own knowledge leaves
them, in both directions until nothing changes, which is what unit
propagation over a clause form decides, and an atom known both to hold and
to fail refutes the region; and, when `producers` recognizes every root as a fact, a rule with a
positive disjunctive head, an atomic choice or a constraint, an atom none of
whose producers can support it, each having an impossible body or another
head held, is cut, a held such atom refutes the region, and an atom held
with exactly one producer able to support it forces that producer's body.
A choice supports its atom whenever its body is not impossible. Every stable model of
the region survives the narrowing, and a refuted region holds none.

The result is `Refuted`, or `Fixed` with whether any atom was decided; the
statistics count charged node visits and producer checks, the propagation
events, and the atoms held and cut. Every event follows a newly learned bit,
so the events are bounded by the bits. A `Narrower` indexes the theory once
and narrows any region of it, the root from knowledge of nothing and a child
from its parent's knowledge. The index
reads each maximal tree of one connective, a clause or a body, as one node
over its operands, a *chain*, when its inner nodes have that one parent and
are not roots; the closure keeps two counters per chain, the operands known
to hold and known to fail, and applies the n-ary rules, a disjunction sure
with one operand and impossible with all, forcing its one open operand when
sure, and the duals for a conjunction (`FormulaChains`). Propagating a new
decision visits its atom occurrences rather than every satisfied clause.
A node false under a frozen mask is either an operand, which fails, or an
inner node whose operands' masks already read it. A region stores held and
cut atom masks; its carried knowledge stores a snapshot of the decisions
already incorporated. The difference supplies new decisions in ascending
atom order. This scan visits `ceil(atoms / 64)` words, and each snapshot
writes that many words, including when no decision is new. Those mask scans
and writes are outside the charged node, parent, producer and open-atom reads
reported as work. The count of parents still unknown that ranks the next
split is kept as nodes become known. `Narrower::narrow_known` narrows from a
`Knowledge` the caller carries from a region to its children and leaves it
closed for them: the knowledge of a region holds in every region inside it
(`known_mono`), so a child learns only the decisions its parent did not know. `Narrower::narrow_frozen_known` narrows a region of the
theory's frozen reduct under a candidate, reading a node false in the
candidate's truth as falsum and applying no support cut, which is the
proper-subset query's narrowing; `FormulaEvaluation::truth` is that mask.
`RegionLimits` bounds charged work, and through it propagation events;
exhausting that ceiling, or a control stop, returns the stop, and the region and
the knowledge then hold what the closure had learned before it, sound but
not closed, which the proposers abandon. A narrowing that does not refute also prefers the open atom with
the most parents still unknown as the region's next split, which the
traversal honours; without a preference it splits the highest open atom. The traversal that splits regions and
covers the tree is `zetesis_cpu::regions::Traversal`, shared with the closure
route; `zetesis-sat` uses it with this narrowing to propose candidates.

Carried knowledge requires the same narrower, theory, producer set and frozen
truth, and a region contained in the ancestor for which that knowledge was
closed. These preconditions are not checked; sibling or foreign knowledge
cannot be reused. The packed `Region` API replaces the old `decisions()`
history with `decided()`'s ascending `(atom, value)` pairs, and `split` consumes
its parent. `snapshot_decided` copies only the available destination prefix;
the narrower supplies all `region.len().div_ceil(64)` words.

`narrow_known_metered` and `narrow_frozen_known_metered` use the same closure with
a caller-owned quota. They request a permit before each charged read and return
`NarrowingAttempt { result, statistics }`, preserving the quota's typed refusal
and the admitted work prefix. Entry control is checked even when no read is
needed; the quota may additionally poll control at every read. The existing
`RegionLimits` methods retain their local-ceiling API.
`narrow_known_reserved` and `narrow_frozen_known_reserved` take a
`NarrowingQuota` instead: they reserve permits in batches of at most
`NARROWING_BATCH`, spend one per charged read, and refund the unspent rest when
narrowing returns. A quota that grants what remains stops narrowing at the same
read as per-read charging and records the same work. SAT injects its search
budget into the reserved methods, so parallel workers hold shared permits
before candidate or frozen-reduct reads, poll control once per batch, and
retain their receipts after failure.
Failed knowledge still must be abandoned. The [metering regressions](tests/integration/region_work.rs)
exercise every prefix of original and frozen narrowing and cancellation.
The [packed knowledge regressions](tests/integration/regions/packed_knowledge.rs) compare
carried and fresh original/frozen closure over 130 atoms and 132 nodes, including
descendant conflicts, zero-work refusals and repeated completed closure.

The three mutable occurrence-count arrays use fixed-length storage: two counts
per chain and one unresolved-parent count per atom. They use `u32` only when it
is narrower than `usize` and the total number of parent incidences fits `u32`;
otherwise they keep `usize`. Every chain operand and every parent counted for
an atom is an occurrence in that same incidence stream, so its length bounds
every counter. Checked additions and decrements preserve the exact count;
width never changes while propagating. This adds no theory-size admission cap
and does not narrow cumulative work statistics. Knowledge clones retain
independent arrays, with unchanged original/frozen ownership requirements.

For A atoms and C chains, the counter payload on a 64-bit host decreases from
8(A + 2C) to 4(A + 2C) bytes when the bound fits. Header layout is counted by
`Knowledge::retained_bytes`; masks, worklists, immutable indexes, scheduler
state and allocator overhead are separate. Each copied Knowledge carries the
same payload reduction. This is a storage model, not an RSS or timing result.
Construction directly allocates the selected width, with no temporary native
counter array; the incidence bound is read from the existing compact index.
The three nonempty arrays still require three allocations per copied Knowledge.
All count reads and updates retain the same charged propagation operations;
storage initialization and copying remain outside that work receipt. Existing
knowledge allocation behavior remains infallible.

The private `compact_counters_reduce_clone_payload` test reports counter and
complete Knowledge storage for 4,096 atoms with four-operand chains. Reproduce
its storage receipt through the normal test harness:

```sh
cargo test -p zetesis-ferraris --lib \
  regions::tests::counters::compact_counters_reduce_clone_payload -- --exact --nocapture
```

The `traversal_copies_only_live_knowledge_arrays` test wraps actual
`Traversal` state clones at 64, 512 and 4,096 atoms. Each fixture leaves eight
atoms free, visits all 256 complete candidates and compares native and selected
counter widths. The receipt counts completed `Knowledge::clone` calls,
initialized array representation bytes and the returned clones' nonempty
backing allocations. It separately records source worklist spare capacity:
`Vec::clone` copies initialized elements, so retained source capacity is not
copy payload. The test verifies the expected counter-width reduction across
every measured split.

```sh
cargo test -p zetesis-ferraris --lib \
  regions::tests::copy_costs::traversal_copies_only_live_knowledge_arrays -- --exact --nocapture
```

This structural receipt excludes inline headers, region masks, shared indexes,
measurement storage and allocator bookkeeping. It is neither an allocator-call
trace nor a timing, RSS or memory-bus measurement. It covers the sequential
`Traversal` fixture, not application-wide clone traffic or parallel workers'
active and queued frontier. Those require separate observations; the retained
bytes reported by a queued frontier do not include active worker state.

Counter tests cover the representable boundary and native fallback, compare
both widths' complete original/frozen closures and split preferences, and
check clone independence. The existing packed-knowledge and metered-prefix
regressions exercise inherited knowledge and refusal receipts. The integer
representation must continue to realize the counts used by `FormulaChains`
and the `Known` relation in `FormulaBounds`; those semantic laws do not by
themselves prove the Rust width conversion. Mutable snapshots remain fully
copied; partial sharing and narrower incidence entries remain separate work.

The immutable node-to-chain map stores `Option<NonZeroUsize>`: a present link
encodes the zero-based chain position plus one, while absence remains `None`.
Both chain construction and its temporary live-chain renumbering map use this
layout. Encoding checks addition; positions in the allocated chain vectors
cannot reach `usize::MAX`, so the representation adds no admission limit.
Consumers decode to the original zero-based positions before reading chains or
counters. Absorption, live-chain order, operand order and charged work stay the
same. On a 64-bit host each map slot occupies 8 bytes instead of the previous
16-byte `Option<usize>`; the retained map has one slot per theory node. Its
vector header and allocation count are unchanged. This index belongs to the
shared `Narrower`, outside the owned `Knowledge::retained_bytes` receipt.

The private chain-link tests cover absence, checked encoding, live-chain
renumbering and original/frozen propagation under equivalent reordered chain
storage. Its storage fixture reports actual map capacities and element widths;
it excludes other index fields, temporary renumbering, knowledge, allocator
overhead and RSS. Reproduce that receipt with:

```sh
cargo test -p zetesis-ferraris --lib \
  regions::tests::chain_links::compact_chain_links_reduce_retained_storage -- --exact --nocapture
```

The immutable index stores five incidence maps in compact row form: parents,
atom nodes and atom operands in `Narrower`, and producers by head and by body
in `Producers`. Each map has one offset vector and one contiguous entry vector.
A row is a borrowed slice, in its original order and with its original duplicate
occurrences. Compaction does not change chain formation, producer identity,
propagation order, split ranking or mutable `Knowledge`. In particular, two
distinct atom nodes for one atom remain two atom-operand occurrences, while the
existing rule for an implication with the same node on both sides still counts
that node once.

For N theory nodes and A atoms, the five maps contain R = 3N + 2A rows in total.
Their logical payload is R + 5 offset words and E incidence words. On a 64-bit
host, replacing each row's 24-byte vector header by its 8-byte offset saves
approximately 16R bytes plus unused incidence capacity, less the small fixed
map-header/sentinel difference. This is a retained-storage model, not a measured
RSS or timing result. Each builder makes two traversals of its unchanged edge
stream plus linear row scans and storage initialization: O(rows + incidences)
placement work, with one temporary cursor word per row. Enumerating the source
streams also scans the nodes or producer records twice per map, so total added
construction remains linear in nodes, atoms, producer records and incidences.
There is no per-row allocation. The existing chain-building algorithm and its temporary operands are unchanged.
The logical indexing receipt remains one visit per theory node; these storage
construction passes are outside that receipt, just as allocation and the
existing chain-building passes were. Propagation's charged reads are unchanged.

`Narrower::try_new` returns `Stop::Allocation` if compact incidence sizes overflow
or their storage cannot be reserved. Operational SAT callers propagate that
refusal. `Narrower::new` remains the infallible convenience constructor and
panics on those refusals. Neither constructor makes the existing chain and
knowledge allocations fallible, and neither adds a cancellation API. Producer
extraction retains its existing control/work contract and also propagates
compact-index allocation refusals. The row tests compare complete ordered
subsequences, including empty rows, shared nodes and duplicates; the existing
original/frozen propagation and quota-prefix tests cover the consumers. The
private `compact_adjacency_uses_less_retained_storage` test reports actual
before/after capacities on an implication ring; its receipt excludes all other
index fields, knowledge, scratch and allocator overhead.

`proofs/Zetesis/AdjacencyRows.lean` proves exact ordered row decoding from
concatenated rows and identical folds over the decoded row. Connecting the Rust
count/prefix/scatter construction to those mathematical lists remains a
representation obligation. `proofs/Zetesis/FormulaBounds.lean` proves the readings sound, the knowledge
sound (`Known`, `known_sound`), and the support cut and the sole-support
rule sound for stable models on the fragment `DisjunctiveSupport` names
(`unsupported_cut`, `sole_support_forces`); the
choice reading and the agreement of the Rust closure with `Known` are Rust
obligations. See [regions API](src/regions.rs) and
[the rule propositions](tests/integration/regions.rs).

## Checked tight producer plans

`TightPlan::compile` extracts normal and atomic-choice producers from every
original root and checks strict positive ranks on a linear-size formula/atom
graph. `TightPlan::certify` validates a proposed rank against the same complete
extraction. Source-analysis labels alone do not authorize either route. Bodies
admit conjunction, disjunction and default negation; each default-negated
interior is candidate-frozen. Constraints and support guards retain their
original truth checks. General disjunction, unnegated body implication and
positive cycles remain on the general reduct path.

`TightPlan::check` evaluates original satisfaction and every present atom's
support. A completed ranked support check proves stability without a subset
query. A false root remains an original-model failure; missing support returns
`Residual` until a caller completes a witness-based rejection. The plan retains
immutable theory identity and has separate producer, dependency, logical-byte
and work limits. An incomplete scan never supplies a successful certificate.

`proofs/Zetesis/TightPlans.lean` proves the formula-level satisfaction/support
characterization for this grammar, including original choices in both branch
orders. It does not prove the Rust extractor, rank implementation or missing
lazy source coverage. Tests compare finite trees and their materialized reducts,
validate rank/root/identity/resource boundaries, and exercise the ordinary
candidate batch protocol.

## Positive producers and original constraints

`PositivePlan::compile_accounted` checks every original asserted root and
computes its least consequences once. Roots may be atomic facts, a positive body
implying one atom, an arbitrary body implying falsum, or falsum. Producer bodies contain
atoms, falsum, And, Or and exactly the truth constant `False -> False`. Positive
cycles are allowed. Choices, default negation and other implications in producer
bodies, and disjunctive asserted heads decline this specialization; a source-analysis label
or dependency projection cannot replace complete original-root validation.

The constructor builds forward incidence for atom and formula vertices. A true
atom signals its occurrences; Or needs one true child and And needs both child
occurrences, including aliases. Each vertex enters the queue once and each
outgoing incidence is visited once. No body is expanded into disjunctive normal
form. The graph is released before exact original evaluation checks roots on the
least interpretation through its first failure. A false producer is an invariant
refusal. The plan retains only its
exact-owner `Interpretation`, first failed original constraint and observations.

`least_consequences()` satisfies the producers. If `failed_constraint()` is
`None`, this interpretation is the unique answer set of the admitted original
theory. Otherwise it is not a model of the full theory, and no answer set exists;
larger classical models may still exist with nonmonotone constraints. The value
is not a solve `AnswerSet` and does not prove source completeness. The
[semantic laws and correspondence obligations](../../proofs/guide/constrained-positive.md)
separate this characterization from Rust queue/index/ownership correctness.

`PositivePlanLimits` independently bounds incidences, named bytes and cumulative
construction, propagation and original-evaluation work. Named bytes count the final header throughout
construction, temporary vector headers and observed capacities; they exclude
shared theory storage, allocator metadata and other stack temporaries. Actual
allocation slack is checked after reservation, so this is not a hard allocator
or RSS cap. A failed attempt preserves admitted work and observed peak capacity
but returns no partial plan. Cancellation, deadline, allocation and limit
failures remain refusals. `compile` delegates to the same implementation while
omitting the failure receipt. No timing gain follows solely from this API.

## Retained native aggregates

`native_aggregate::Group::new` imports already OR-coalesced tuple keys and
term-valued guards into one canonical vocabulary. Owned `Value` descriptions are
construction input; reductions borrow `TermRef` values. The alternative
`GroupData::new` admits metadata over an existing `CatalogRead` without importing
its payload. `bind_with` checks vocabulary identity and the required prefix before
returning a `GroupRef`. The caller retains that external vocabulary.

Both forms preserve complete-key equality, original occurrence order, empty
keys, typed ASP ordering and the original/frozen eligibility contract. Equal
complete keys are refused, even if their conditions coincide. Scalar, Rayon and
wgpu callers consume the same borrowed group; device preparation adds its own
numeric capability checks. Admission alone establishes no formula truth or
answer-set membership.

`AdmissionLimits::max_bytes` retains the logical transferred-carrier measure.
The separate `max_storage_bytes` bounds named canonical capacities, metadata,
duplicate-check scratch and publication overlap. Owned admission includes its
vocabulary; borrowed admission excludes the caller's catalog. Caller ingress,
shared theory payload and allocator bookkeeping are outside this dimension.
`Statistics::storage_bytes` and `storage_peak_bytes` report canonical admission
capacity separately from the legacy logical fields. These are bounded component
receipts, not process RSS. Refusal returns its typed cause and accounted prefix.

## Finite scalar aggregates

`append_aggregate(nodes, elements, comparison, bound, limits, control)` appends a
formula to an existing topological `Vec<Node>`. It returns an absolute root index,
appended-node count, translation profile and statistics. The root is not asserted
and no surrounding rule is added. Every new node is an ordinary connective;
compilation introduces no semantic atoms or separate support rules.

Each `AggregateElement { weight: i32, condition: usize }` represents one distinct
whole tuple and its existing eligibility formula. Use weight one for `#count` and
the integer first tuple component for `#sum`, `#min` or `#max`. The caller must group equal complete
tuples and OR all their eligibility conditions before calling; equal weights are
not enough to group tuples. Arbitrary formula conditions, including default
negation, remain intact. In particular, an eligibility formula `p or not p` must
not be replaced by true.

The constructor admits a fixed `i64` scalar guard and `Eq`, `Ne`, `Lt`, `Le`, `Gt`
or `Ge`. The caller handles source local bindings, tuple expansion, assignment
values, multiple guards, head choices and source origins. Source intervals in a
guard, undefined/nonnumeric weights, `#sum+` and theory aggregates are not
implemented. Numeric `#min` and `#max` use the separate constructor below.
Default negation is added afterward as
`Implies(root, falsum)`; it must not be implemented by inverting the comparison.

The general translation uses one implication for every failing subset Δ:

```text
AND over failing Δ: (AND of conditions in Δ) -> (OR of conditions outside Δ)
```

Empty conjunction/disjunction mean true/false. This is the finite formula
translation from [Abstract Gringo, corrected v2, §4.7](https://arxiv.org/pdf/1507.06576v2).
Tuple grouping preserves each tuple's eligibility in both the candidate and the
frozen reduct because disjunction preserves the union of its alternative supports.
Grouping does not justify simplifying conditions by classical truth alone.

For nonnegative weights, a dynamic program constructs shared formulas `G(k)` for
sum ≥ k using only AND/OR and constants. Each cell combines excluding an element
with including it and the previous row's residual threshold. This is a compact
factorization of the monotone aggregate formula. `>` shifts the threshold; `<`
and `<=` negate the corresponding monotone threshold. Equality conjoins `G(k)`
with `not G(k+1)`. Not-equal uses `G(k) -> G(k+1)`, preserving the implication
needed at a nonconvex guard.

This follows the reduct equivalence and monotone/antimonotone laws in
[Ferraris, §§3.2–3.3, Propositions 12–13](https://arxiv.org/pdf/0812.1462).
For the not-equal specialization, frozen condition truth can only remove eligible
elements. If the candidate sum is below k, the implication's antecedent is frozen
false; if it equals k, the entire root is frozen false; if it exceeds k, the
implication tests whether the reduct-eligible sum is below or above k. This
argument requires nonnegative weights. Signed weights always use the full subset
implications and are never complemented and shifted by classical algebra.

For example, `p :- #count {1:p} != 0.` has only the empty stable model, whereas
`p :- not #count {1:p} = 0.` also has `{p}`. Both are regression cases. The
constructor is source-backed executable code; its aggregate translation has not
been refined into Lean.

## Shared comparison families

`FormulaNodes` owns a growing node vector when several compilations share it.
Its scalar, family and extremum append methods use the same compilers as the
free functions, while retaining the extent of completed prefix validation.
Read access is immutable; appends leave an unchecked suffix, and truncation or
suffix extraction clamps the retained extent. Reusing that extent changes
validation work, not formula nodes, roots or aggregate semantics. Every call
still checks limits, elements and cancellation. A failed transaction removes
its appended nodes and reports its spent work. Consuming `into_vec()` releases
the owner; constructing another owner from those nodes requires validation again.
Raw-vector free functions continue to validate the whole prefix each time.

`append_aggregate_family(nodes, elements, guards, limits, control)` compiles an
ordered list of `AggregateGuard { comparison, bound }` against one identical
coalesced element family. It returns one root per guard, preserving order and
duplicate requests. `AggregateFamilyLimits` contains the existing aggregate
limits plus `max_guards`, which bounds the output-root vector independently.
The original single-guard API and its accounting contract remain unchanged.

For nonnegative weights, the family constructs one threshold table through the
largest needed nonconstant threshold. Every requested inequality or equality
then refers to its final row. Both thresholds of an equality share this table;
`!=` retains the implication between the corresponding thresholds. Thresholds
at most zero or above the total weight use constants and do not enlarge the
table. This preserves the same failing-subset and frozen-reduct semantics as
individual compilation, while avoiding repeated DP work. The complexity is
proportional to elements times the largest needed threshold, plus requested
guards and prefix validation. It remains pseudopolynomial.

Signed families share validation and constants, then compile the full
failing-subset formula for each guard. Work and subset limits apply cumulatively
to the whole transaction. A later guard that exhausts a limit rolls back every
new node; callers never receive a partial root family. Empty families still
validate the prefix and conditions and poll control, but append no nodes.
Temporary-state accounting counts DP cells or subset bits, excluding returned
roots and DAG storage. Root storage is capped by `max_guards`, whose default is
4,096; existing aggregate defaults are unchanged.

The caller must establish that tuple keys and eligibility conditions are the
same for every requested guard. In source assignment, this requires a complete
fixed support universe and independence from the changing assignment target.
This numeric constructor neither checks source binding independence nor proves
a source cache valid. Interning an appended DAG remains a separate transform.

Eight portable family tests compare 6,300 exhaustive guard configurations and
192 generated families with an independent full failing-subset definition, for
all two-atom candidates and tested reduct interpretations. Further checks cover
guard order, duplicate guards, extreme bounds, zero weights, inclusive limits,
empty families, cancellation and rollback after a later signed guard stops.
A 16-element count family covering every equality and a duplicate request uses
one threshold table and less than one fifth of the work of separate calls.

## Numeric minimum and maximum

`append_extremum(nodes, elements, extremum, comparison, bound, limits, control)`
appends a `Min` or `Max` comparison over the same coalesced tuple elements. The
numeric first component may be any `i32`, including negative and extreme values.
`ExtremumBound` is `NegativeInfinity`, `Number(i64)` or `PositiveInfinity`, with
conversions from `i32` and `i64` for direct numeric arguments. Infinite guards are
distinct from the smallest and largest integers. The empty minimum is `#sup`;
the empty maximum is `#inf`, following
[Abstract Gringo, corrected v2, §2.1](https://arxiv.org/pdf/1507.06576v2).
Nonnumeric or explicitly infinite element values are outside this numeric API.
Source assignment and the discovery of possible assignment values remain caller
obligations; this constructor alone admits no additional source program.

For maximum, `G` is the disjunction of conditions whose value is at least the
guard, and `H` uses strictly greater values. For minimum, these tests reverse:
`G` uses values at most the guard and `H` uses strictly smaller values. At the
appropriate empty-set sentinel, `G` includes true. The remaining comparisons
use these two monotone formulas:

| Comparison | Maximum | Minimum |
| --- | --- | --- |
| `>=` | `G` | `not H` |
| `>` | `H` | `not G` |
| `<=` | `not H` | `G` |
| `<` | `not G` | `H` |
| `=` | `G and not H` | `G and not H` |
| `!=` | `G -> H` | `G -> H` |

This is a linear factorization of the full failing-subset formula, justified by
the same monotone/antimonotone laws above. For `!=`, frozen conditions can only
remove eligible elements. When `G` is candidate-false its implication freezes
to true; at equality the entire root freezes to false; when both `G` and `H`
hold, the implication retains exactly the forbidden equality test on the
reduct-eligible elements. For minimum this argument follows increasing witness
sets for `<=` and `<`, rather than the order of numeric results. It requires no
sign restriction on the finite values. Default negation still wraps the result
with implication to falsum. For example,
`p :- #min {1:p} != #sup.` has only the empty stable model, while replacing its
body with `not #min {1:p} = #sup` also allows `{p}`.

The extremum profile charges prefix validation and a linear input scan. It adds
at most `2*n + 4` nodes and retains no dynamic threshold rows or subset carrier;
`max_states` and `max_subsets` can both be zero. Fixed local accumulators are not
counted as dynamic state cells. Signed extremes introduce no threshold shift or
integer negation. Work, nodes, elements, control polling, source-origin capacity
and transactional rollback follow the same contract as `append_aggregate`.

Seven portable tests check 12,600 exhaustive configurations and 192 generated
configurations against both direct selected-value semantics and the independent
full failing-subset formula. They compare every two-atom frozen candidate and
tested interpretation, including interpretations outside the candidate. Separate
cases cover both infinities, empty and tied values, composite and default-negated
conditions, all integer endpoints, malformed DAGs, exact budgets, cancellation
and a 4,096-element linear compilation with no subset or state budget.

Three optional equivalence tests compare 840 complete recursive source programs
with clingo, covering all comparisons, both infinities, zero/one/two default
negations, tuple coalescing, ties, interior numeric endpoint neighbours and
extreme element values. A fourth test characterizes the six known discrepancies
below; its successful execution does **not** count as six equivalence passes.

```sh
cargo test -p zetesis-ferraris --test integration -- --ignored extrema_clingo::
```

### Known clingo numeric-endpoint compatibility gaps

The direct kernel follows exact finite integer and infinity semantics. clingo
5.8.2 disagrees for the following original source shape:

```text
p :- #FUNCTION {W,k:not p} OP W.
```

| Function | `W` | `OP` |
| --- | --- | --- |
| `min` or `max` | `-2147483648` or `2147483647` | `!=` |
| `min` | `2147483647` | `>` |
| `max` | `-2147483648` | `<` |

Each of these six cases has `{}` and `{p}` under the full failing-subset
definition; clingo 5.8.2 returns only `{p}`. Adjacent interior guards agree.
The operational cause has not been established by this kernel work, and no
integer wraparound has been introduced to imitate the observation.
[The boundary evidence](tests/fixtures/extrema-clingo-5.8.2-boundaries.json)
retains all 24 endpoint/comparison probes, their exact source, raw clingo output,
ground text and separate equivalence/mismatch classification.
`known_clingo_integer_endpoint_gaps_are_reported_separately` reads the six known
mismatches from that record, checks each family against the kernel and against
clingo, and requires reassessment if the oracle changes.

A frontend promising clingo compatibility must refuse evaluated numeric min/max
guards at `i32::MIN` and `i32::MAX` until an explicit compatibility policy is
implemented. This restriction does not apply to the direct mathematical kernel,
nor does it identify `#inf`/`#sup` with numeric endpoints. No universal source or
clingo compatibility claim follows from the successful kernel tests.

## Aggregate compilation bounds

`AggregateLimits` independently caps input elements, absolute total DAG nodes,
charged work, peak temporary state cells and enumerated subsets. The threshold
profile uses two rows of threshold indices and takes work proportional to the
number of elements times the scalar threshold; its arithmetic complexity is
pseudopolynomial. Large weights or bounds can trigger `StateLimit` even with few
elements. Signed compilation is exponential and admits the entire subset count
before enumeration; `max_subsets` includes the empty subset. The threshold profile
does not enumerate subsets. A 64-element exact-eight count is covered by tests
with a zero subset budget.

Existing edge topology and element indices are checked under the work budget.
`Theory::new` remains responsible for validating atom-universe indices. Every
charged operation polls shared cancellation/deadline control; storage reservations
and arithmetic are checked. The state count excludes DAG storage and allocator
overhead. All ceilings are inclusive, and zero is a real ceiling.

On failure, appended nodes are removed and the original prefix and length are
preserved; vector capacity may have changed. Error statistics report work done
before rollback. Callers maintain origin tables separately and can clamp the total
node limit to the number of origins they can admit. Any stopped compilation is a
refusal, never an unsatisfiable formula.

Eight portable aggregate tests cover 6,300 exhaustive configurations and 192
generated configurations against independent frozen-condition semantics, checking
all two-atom candidate/tested worlds, including tested worlds outside the
candidate. They also cover empty aggregates, zero/signed/extreme weights, repeated
condition IDs for distinct tuples, exact budgets, rollback, bad DAG edges and
control interruption. Two optional tests compare 152 complete small recursive
source programs with clingo, including OR-coalesced tuples and default-negated
aggregates:

```sh
cargo test -p zetesis-ferraris
cargo test -p zetesis-ferraris --test integration -- --ignored aggregate_clingo::
```

The optional subprocesses use temporary files, a five-second deadline and a
64-KiB captured-output ceiling. clingo is only an independent test oracle.
