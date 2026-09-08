# Eager and lazy grounding optimization

The most promising near-term grounder improvements are bounded columns of
bindings, incremental relation rounds, and join plans that account for which
arguments are actually bound. They address different costs: constructing and
evaluating a binding, reconsidering an old binding, and discovering a binding.
They can share a relational contract while retaining different CPU and GPU
representations. None requires replacing reduct-based answer-set checking.

This report evaluates those opportunities against Rust source
`0b72472f4093bb8149940c3c7e68e290cc8bd33e` and the accompanying September 8, 2026
design and verification records. Line anchors refer to that source revision.
Literature was checked through September 8, 2026. Recommendations are research
hypotheses, not implemented features or forecasts of speedup. Intended-language
closure remains the next implementation priority; this inventory supports the
focused optimization work that follows. Reproducible evidence will also help
eventual public dissemination, without creating a separate release workstream.

This report extends the existing
[foundation opportunities](../design/foundation-optimization-opportunities.md)
and [GPU Datalog study](../design/gpu-datalog-20260906.md). Several mechanisms
were already proposed there; the contribution here is a deeper comparison,
updated literature, source-level boundaries and a bounded experiment order.

## 1. Architecture and evidence

### The eager path

The eager formula path first constructs possible support, activates objective
information, instantiates rule formulas, adds coherence and support conditions,
and validates the resulting theory. These phases are visible in
[formula_ground.rs:25](../../crates/zetesis-themelios/src/formula_ground.rs#L25).
Possible support encloses potential atoms; it is neither a candidate answer set
nor a stable-model certificate. Optimizing its positive relational computation
must preserve that enclosure and the subsequent original/frozen formula
interpretation.

Support already has per-argument indexes. Each predicate relation holds full
`Atom` rows and a map from each argument value to row positions. Probing chooses
the shortest available posting list; the matcher still checks the other
arguments. This is indexed row storage, not yet a numeric column execution
engine. Index entries, row construction and work have explicit limits.
See [RelationRows:92](../../crates/zetesis-themelios/src/formula_support.rs#L92)
and [probe:134](../../crates/zetesis-themelios/src/formula_support.rs#L134).

The join enumerator sorts ordinary positive body patterns by relation size and
backtracks with binding frames. It does not build a materialized intermediate
relation after every binary join. Ready comparisons already reject partial
bindings. An undefined expression encountered on an incomplete prefix can be
deferred because that prefix may never extend to a complete relational
binding; resource failures are not silently deferred. These distinctions rule
out treating indiscriminate early arithmetic evaluation as a safe optimization.
See [join construction:390](../../crates/zetesis-themelios/src/formula_support.rs#L390)
and [partial filters:766](../../crates/zetesis-themelios/src/formula_support.rs#L766).

Two other optimizations are present. The checked scalar evaluator reuses empty
value storage between expressions, while fixed-head rule factorization
represents independent witness components as a conjunction of alternatives.
The latter can avoid a cross product of witnesses without inventing semantic
atoms. Proposals below extend these facilities; they do not count either as
new work. See [evaluation.rs:21](../../crates/zetesis-themelios/src/formula_support/evaluation.rs#L21)
and [formula_factor.rs:19](../../crates/zetesis-themelios/src/formula_factor.rs#L19).

### The lazy path

The relational lazy oracle grows a closure under a frozen candidate's gates.
Each round constructs predicate relations from the current closure, visits
templates, collects consequences and tests for completion. Although it gathers
new atoms in a delta, its rule inputs are not yet restricted to newly changed
rows. At completion, constraints and the required seed agreement still matter.
See [oracle.rs:178](../../crates/zetesis-cpu/src/oracle.rs#L178).

Bound leading arguments already narrow a relation to a binary-searched window.
The order is `Atom`/`Value` storage order, not an interchangeable substitute for
ASP term comparison. A changed parent binding requires a new window.
See [window.rs:15](../../crates/zetesis-cpu/src/oracle/window.rs#L15).
Shared scans already use world-membership masks to prevent a union of rows
from different candidates from fabricating a witness in any individual world.
Adding another redundant mask is not a new optimization.

Source scanning offers filter-valid instances from an immutable snapshot. Its
completion means that the snapshot has been exhausted, not that the final
closure or candidate space is exhausted. Repeated offers remain repeated
offers; callback failure or interruption cannot become successful completion.
The scanner retains an index, a binding frame and one current instance rather
than a complete ground-rule store. These are explicit contracts in
[source.rs:145](../../crates/zetesis-cpu/src/oracle/source.rs#L145).

Metal currently accelerates bounded checking of those source instances while
host code performs the source joins. The library boundary admits a source
provider and batches in
[GpuLazyOracle::check_batch_with_source:153](../../crates/zetesis-wgpu/src/lazy.rs#L153).
This is real lazy execution with device work for its supported relational
profile. It does not imply that every eagerly admitted formula program already
has a lazy execution path. In particular, translating a source numeric tile
into general lazy arithmetic requires the corresponding lifted language
representation; the existing lazy filters are a smaller initial target.

### What measurements establish

The [aggregate-primitives qualification record](../verification/aggregate-primitives-tranche-20260908/README.md)
records 94 clean corpus cases, 1,931 selected model occurrences, 24 selected
upstream cases, 25 physical Metal tests, and workspace coverage of 93.5816% when
the matching device profiles are included. These establish scoped correctness
and coverage evidence. They do not establish faster grounding, general lazy
formula coverage, or a fully GPU-hosted solver. Timing evidence belongs to its
own frozen build and campaign.

The subsequently completed [Metal measurements](../verification/aggregate-primitives-tranche-20260908/measurements/README.md)
retain all 7,020 samples in two separate operation populations. Resident native
aggregate reduction beats scalar in 8 of 45 case medians and four-worker Rayon
in none. At 4,096 tuples and 128 occurrences, host eligibility acquisition takes
0.964–1.044 ms separately from resident reduction's 1.020–1.106 ms. Shared
acquisition is therefore a concrete next hypothesis, not a measured improvement.
The lazy comparison reduces allocation-bearing submissions from 96 to 80 across
24 GPU case/route cells, with unchanged source work, transfers and peak requested
payload. Timing changes remain modest and variable. Existing residency and input
retention are baseline mechanisms to extend; implementing them again is not a
new optimization. These campaigns do not measure whole-solver speed or RSS.

Two recent experiments are particularly useful negative evidence:

| Experiment | Observation | Consequence for planning |
| --- | --- | --- |
| [Numeric prefix](../verification/numeric-prefix-20260908/measurements/README.md) | Withdrawn after a matched 12-source comparison. SEND's formula-admission median with optional observation disabled was 17.945375 ms before and 18.028604 ms after. Some queens encodings became slower. | Repeating scalar arithmetic specialization under a new name is insufficient. A new experiment must change column traversal, binding materialization, or demonstrable reuse and measure those changes separately. |
| [Source count plans](../verification/source-count-plans-20260908/measurements/README.md) | The largest admitted fixture reduced decisions from 93 to 80 but increased recorded search work from 147,555 to 153,968. All six queens encodings returned `NoPlan`. | Fewer decisions do not establish a speed improvement. Plan discovery and eligibility must be measured; a declined plan is not evidence that no valid specialization exists. |

The numeric campaign measured admission, not complete CLI solves or RSS. The
count-plan campaign did not establish ordinary CPU/Metal speedups. They should
remain available alongside successful experiments. The
[grounder comparison](../design/grounding-compared-with-clingo.md) also makes an
important baseline distinction: clingo's grounder already performs sophisticated
instantiation. Comparisons against a hypothetical naive Cartesian grounder
would misstate both systems.

## 2. Transferable research

### Columns, vectors and physical layouts

Sun and colleagues' *Column-Oriented Datalog on the GPU* describes FVlog's
separate value columns, row-position indexes and late materialization of join
results. A counting phase and prefix sums allocate output positions before
writing them. The useful transfer is this separation of logical rows from
physical movement, rather than the paper's CUDA speedup figures. Raw columns,
indexes and merge work all consume space. [Sun et al., 2025](https://arxiv.org/abs/2501.13051)

GPUlog uses hash-indexed sorted storage and semi-naive relation processing; its
published ASPLOS study examines both relational kernels and the surrounding
materialization costs. The GDlog artifact belongs to this same line of work,
so it should not be counted as independent confirmation. Its CUDA implementation
is an algorithmic reference, not a portable wgpu component.
[Sun et al., 2025, *Optimizing Datalog for the GPU*](https://arshovon.com/publications/ASPLOS_2025.pdf)

On CPUs, X100 motivates bounded vectors between operators: enough work to
amortize per-row interpretation without materializing every complete column.
A later controlled comparison of compiled and vectorized execution found
different strengths for cache misses and instruction counts. Neither supports
assuming that vectors always beat the current fused scalar loop.
[Boncz, Zukowski and Nes, 2005](https://www.cidrdb.org/cidr2005/papers/P19.pdf),
[Kersten et al., 2018](https://ir.cwi.nl/pub/28470/28470.pdf)

A useful counterexample is Lu and Kumar's WebGPU relational-algebra poster:
its reported WebGPU configurations trail the CPU and CUDA references on the
listed transitive-closure workloads, with sorting particularly costly. This is
limited poster evidence, but directly cautions against equating portable GPU
execution with a performance win.
[Lu and Kumar, 2025](https://www-new.evl.uic.edu/documents/jiaxin.pdf)

For zetesis, **normalization** and **transposition** therefore need different
contracts. Normalization changes an expression or relation plan and requires
semantic equivalence. Transposition changes a representation and requires
lossless reconstruction, including identity and multiplicity where observable.
The sparse-tensor dialect's separation of logical dimensions and storage
levels is a useful design analogy. It does not justify importing MLIR, C++, or
a tensor dependency into this Rust/Lean project.
[MLIR SparseTensor dialect](https://mlir.llvm.org/docs/Dialects/SparseTensorOps/)

### Join selection, factorization and incremental work

Automatic Datalog index selection can cover known equality-search patterns
with a smaller set of indexes. Soufflé's join-optimizer work separately uses
profile information to improve join ordering. These are stronger starting
points than indexing every imaginable argument permutation or relying only on
whole-relation length.
[Subotić et al., 2018](https://psubotic.github.io/papers/p141-subotic.pdf),
[Arch et al., 2022](https://souffle-lang.github.io/pdf/lopstr2022.pdf)

Worst-case-optimal joins address conjunctive queries for which intermediate
binary joins can exceed the query's worst-case output bound. Leapfrog Triejoin
uses ordered seek/intersection operations, while Free Join combines traditional
and multiway plans in one framework. These results justify a selective plan
alternative, not replacing every streaming backtracking join.
[Ngo et al.](https://arxiv.org/abs/1203.1952),
[Veldhuizen](https://arxiv.org/pdf/1210.0481),
[Wang, Willsey and Suciu, 2023](https://www.mwillsey.com/papers/freejoin)

The 2026 SRDatalog preprint extends this direction to GPUs with flat storage,
multiway traversal, incremental index maintenance and skew-sensitive scheduling.
Its inspected April 23 version is a preprint; placeholder publication metadata
in the document is not a verified conference acceptance. Its architectural
ideas warrant a future experiment, but its Datalog/CUDA results do not prove
ASP/Metal performance or completeness.
[Sun et al., 2026](https://arxiv.org/html/2604.20073v2)

Factorized databases represent common witnesses through nested unions and
products instead of repeating every flat result. This connects closely to
zetesis's existing rule factorization. Differential dataflow addresses another
axis: maintaining results across changes and nested iterations. It suggests
careful epoch and update accounting, while its general machinery would be a
large addition to this solver.
[Olteanu and Závodný, 2012](https://www.cs.ox.ac.uk/dan.olteanu/papers/oz-icdt12.pdf),
[McSherry et al., 2013](https://www.cidrdb.org/cidr2013/Papers/CIDR13_Paper111.pdf)

### ASP-specific restrictions on transfer

Lazy-grounding research documents the trade between lower materialization and
the information available for search. Lazy normalization additionally studies
the difficulty of handling aggregates before their complete ground structure
is known. These support evaluating degrees of eagerness and retaining native
aggregate structure. They do not require adopting another solver's search
algorithm.
[Taupe, Weinzierl and Friedrich, 2019](https://arxiv.org/abs/1903.12510),
[Bomanson, Janhunen and Weinzierl, 2019](https://repositum.tuwien.ac.at/handle/20.500.12708/58118)

An especially relevant new result is Hanisch and Krötzsch's static-filtering
work. It generalizes backward propagation of consumer restrictions and gives
tractable approximations to an expensive general analysis. Its ASP extension
establishes a stable-model bijection for admissible rewritings; the map removes
filtered atoms and preserves specified outputs. This is stronger than a claim
about one query answer, but it is not literal equality of all original models.
Using it for zetesis requires reconstruction and a bridge to the actual
aggregate, objective and checked-arithmetic language.
[Hanisch and Krötzsch, ICDT 2026](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ICDT.2026.5),
[full proof, theorem 22](https://arxiv.org/html/2601.05108v1)

The ngo project is another appropriate source of transformation candidates.
Its syntactic optimizer is not itself a correctness certificate for a zetesis
pass. A rewrite must preserve the actual source semantics or have a justified
model reconstruction. Classical equivalence alone is insufficient: stable
models depend on reduct behavior, and strong equivalence characterizes
replacement in arbitrary program contexts.
[Potassco ngo](https://potassco.org/ngo/),
[Lifschitz, Pearce and Valverde, 2001](https://www.cs.utexas.edu/~vl/papers/ht.pdf)

## 3. Ranked opportunities

The ranking balances likely benefit, semantic risk and the size of a decisive
experiment. It is not a claim that every item should be implemented. Eager and
lazy paths should share logical operations where their contracts coincide;
different admitted languages and failure boundaries remain explicit.

| Rank | Opportunity | Eager applicability | Lazy applicability | Main time and memory hypothesis |
| --- | --- | --- | --- | --- |
| 1 | Bounded binding columns | Numeric/comparison-heavy formula joins | Existing relational comparisons first; broader arithmetic after admission support | Less `Value` construction and repeated interpretation; bounded scratch may increase temporary bytes while lowering allocation traffic. |
| 2 | Delta relation rounds | Positive possible-support growth under a sound schedule | Closure within one frozen candidate or world batch | Avoid old joins; retain delta/index metadata instead of rebuilding all relation views. |
| 3 | Binding-aware indexes and join order | Replace coarse relation-length choices selectively | Reuse stable indexes and choose useful bound-prefix orders | Fewer probes and unnecessary indexes; compilation and maintenance may dominate small relations. |
| 4 | Shared keyed eligibility acquisition | Aggregate/objective lowering and independent witnesses | Repeated groups across source instances and worlds | Reuse condition evaluation and avoid flat witness products; cache scope and retained state must be bounded. |
| 5 | Semijoin and multiway alternatives | Selective conjunctive body components | Expensive recurring source joins | Avoid inconsistent prefixes; extra index/filter storage can lose on simple or changing relations. |
| 6 | Checked range analysis and prefix reuse | Numeric binding plans and finite generation | Admitted lifted numeric filters | Eliminate proven-impossible bindings or repeated expressions; analysis and caches have a measurable price. |
| 7 | Resident GPU relational pipelines | Large repeatable numeric or keyed tiles | Shared source snapshots with many worlds | Amortize conversion, use parallel count/scan/materialize; device buffers may increase total resident memory. |
| 8 | Certified source filtering and normalization | Conservative program-to-program planning | Smaller source programs before lazy execution | Reduce the binding space itself; strongest reductions have broader proof and reconstruction obligations. |
| 9 | Cross-candidate incremental execution | Repeated related grounding sessions later | Related frozen candidates and adaptive eager regions | Reuse established work, at the cost of retraction, provenance and epoch complexity. |

### 1. Bounded binding columns

Start with an internal view of an unchanged binding stream. A tile carries
stable row positions, selected integer columns and a validity mask. Evaluate a
small admitted expression plan over that tile, then construct full terms and
formulas only for surviving rows. Structured or nonnumeric values retain the
existing path; classification failure declines this physical plan rather than
rejecting an otherwise supported program.

This differs materially from the withdrawn numeric-prefix experiment. It
tests late materialization and operator traversal across multiple bindings,
not only a different implementation of one scalar expression. Compare scalar
evaluation, scalar evaluation over columns, and Rayon over independent tiles
before attempting a shader. Compiler vectorization is a measured possibility,
not a property implied by an array type.

The first plan should preserve binding order and existing error reachability.
Speculative evaluation of an incomplete prefix must not expose an error that
the reference path would never reach. A tile can retain each row's result or
located fault and commit according to the established traversal contract.
Memory accounting includes column capacity, masks, references to original
values and any buffered results. SEND is a motivating workload, not an
admission predicate: rules with the same operations must receive the same plan.

### 2. Delta relation rounds

For a positive body with several recursive relation occurrences, newly enabled
bindings must contain at least one new row. Semi-naive evaluation can assign
each such binding to a distinguished delta occurrence, using old/current
relations elsewhere so that all bindings are covered. Repeated occurrences
of the same predicate still occupy different body positions. Deduplicating
facts is separate from preserving the scanner's observable instance offers.
An incremental operator must advertise coverage of its declared delta, not
masquerade as the existing API's exhaustive scan of the entire snapshot.
Within that declared domain, required duplicate occurrences remain accounted
for even when the resulting consequence is a set-valued fact.

The smallest credible target is one frozen relational lazy epoch. Candidate
gates remain fixed while positive closure grows, which supplies a clear
monotone boundary. Bootstrap rules with no positive prerequisites still run.
Completion requires the appropriate final constraint and seed checks; skipping
all work merely because a particular delta is empty is insufficient.

After that bridge is established, investigate eager support rounds. Their
monotone possible-atom enclosure offers a related opportunity, but general
aggregate dependencies, domain producers, generators and objective activation
need their own schedule. Predicate SCCs help order dependencies; they do not
by themselves prove finite generation or safe aggregate reordering. Updating
relation/index storage incrementally is a second experiment, separate from
changing which bindings are visited.

### 3. Binding-aware indexes and join order

Collect the equality-search patterns of a prepared rule, then choose a bounded
set of indexes that covers them. Eager support currently pays for every
argument's postings; some may never be useful for a given program. Conversely,
a compound lookup or intersection can be worthwhile when no individual
posting is selective enough. Index construction, updates, comparisons and
saved probes all belong in the cost comparison.

For lazy scans, an index must remain valid for its immutable snapshot or be
explicitly advanced to a new epoch. Avoid rebuilding several permutations of
all rows on every closure round. A stable row catalog plus incrementally
maintained position lists is one hypothesis; small relations should retain
the simple window implementation.

Join ordering can use currently bound arguments, relation cardinalities and
observed posting lengths. Keep planning bounded and deterministic, with a
fallback order that preserves coverage. Changing enumeration order also
changes which resource limit or located arithmetic error might be reached
first; specify that contract rather than accidentally coupling it to a hash
table's iteration order.

### 4. Shared keyed eligibility acquisition

Current native aggregate groups retain complete tuple keys and eligibility
formula identities. Acquisition evaluates the retained formula prefix and
projects tuple activity; reduction then consumes that activity. The CPU
primitive and its Metal counterpart are qualified, but they are not yet a
replacement for ordinary expanded aggregate formulas. See
[Group::eligibility:107](../../crates/zetesis-ferraris/src/aggregate/native/eligibility.rs#L107)
and [Group::reduce:138](../../crates/zetesis-ferraris/src/aggregate/native/reduction.rs#L138).

A concrete opportunity is to evaluate the union of required condition nodes
once for a batch of groups, with dependency slices computed once per immutable
theory. Compare this with repeated full-prefix acquisition. A topological
slice must include every predecessor needed for both original and frozen
evaluation; a set of visible tuple nodes alone is not enough. Its construction
and retained storage count against the benefit.

Whole-key grouping can then support contiguous eligibility segments and
factorized witnesses. Duplicate aggregate tuples contribute once after their
conditions are OR-coalesced; duplicate numeric weights with different remaining
tuple fields are distinct tuples. Objective keys and priority-presence rules
have a separate contract. Share physical grouping operations only where the
caller supplies the correct logical identity. General head aggregate closure
is an especially useful prerequisite because atom permission, tuple activity
and the aggregate's value must stay distinct.

### 5. Semijoin and multiway alternatives

A semijoin removes rows that have no matching row in another relation without
constructing their combined tuples. For suitable acyclic conjunctive
components, a reduction schedule can remove many doomed witnesses before
enumeration. Multiway intersection is a complementary option for cyclic joins
such as a triangle with shared variables. Neither is automatically useful for
a two-relation selective lookup.
[Yannakakis, 1981, author copy](https://www.researchgate.net/profile/Mihalis-Yannakakis/publication/200034379_Algorithms_for_Acyclic_Database_Schemes/links/5745c2a708ae9f741b430b62/Algorithms-for-Acyclic-Database-Schemes.pdf)

The required classification is the rule's variable/relation hypergraph.
Predicate dependency acyclicity and program tightness describe different
structures and must not be used as substitutes. Begin with local positive
components whose variables and finite rows are already admitted. Do not use
temporary absence from a still-growing lazy relation as permanent proof that
a source row is irrelevant.

Free Join is particularly interesting as an architectural precedent because
it avoids an all-or-nothing choice between binary and multiway processing.
Its full-query treatment permits bags, so its physical ideas still need a
bridge to each of zetesis's distinct set and occurrence contracts.
For zetesis, first compare a single admitted multiway plan against the actual
backtracking join on skewed and unskewed data. Include high fan-out cases where
the existing factorization already avoids flat witness expansion. More
elaborate GPU tries are justified only after this comparison.

### 6. Checked range analysis and prefix reuse

The independent domain-analysis crate already computes conservative finite
enclosures with explicit uncertainty and limits. It does not yet provide
general arithmetic interval narrowing. Abstract interpretation offers the
right discipline: derived bounds must contain all concrete possibilities;
an inconclusive analysis means unknown, not an empty domain.
[Cousot and Cousot, 1977](https://www.di.ens.fr/~cousot/COUSOTpapers/POPL77.shtml)
See [zetesis-domain::analyze:46](../../crates/zetesis-domain/src/analysis.rs#L46).

Start with intervals and signs for a small operation set, including checked
intermediates. A proof that `X + Y` stays in range permits a narrower physical
evaluation plan. A proof that a comparison cannot hold can remove a binding
only when the existing source error semantics also permits doing so. Division
by zero, minimum-integer division by minus one, remainder and exponentiation
need operation-specific laws. Reassociation of source arithmetic is not a
default consequence of mathematical equality.

Prefix reuse is another mechanism: memoize an expression whose free variables
have an unchanged assignment while later relational variables vary. Keys must
include operation identity, complete values, variable scope and relevant
program/snapshot identity. Cache success or failure only within a contract
that preserves reachability. Begin with bounded per-frame storage rather than
a global cache. The negative numeric experiment makes reuse counts and cache
costs necessary evidence before claiming this worthwhile.

### 7. Resident GPU relational pipelines

Offload a bounded chain of useful operators rather than sending every scalar
comparison separately. One candidate chain is numeric-column filtering,
compaction, whole-key grouping and eligibility projection. Another is a lazy
snapshot's shared positive join followed by per-world membership filtering.
Logical operations retain their identities while physical tiles may be
transposed from rows-by-fields to fields-by-rows or rows-by-world-words.

Count/scan/materialize requires checked output counts, checked prefix sums,
bounded allocation and complete decoding. It cannot assume that input size
bounds join output size. Stable output positions or a declared canonical merge
are needed wherever occurrence order is observable. Workgroup-local barriers
do not provide global synchronization; multi-stage scans need explicit
dispatch dependencies. Subgroup width, subgroup operations and integer
capabilities must come from adapter features, not CUDA assumptions.
[WGSL, August 31, 2026 draft](https://www.w3.org/TR/2026/CRD-WGSL-20260831/)

The current 64-lane aggregate reduction is a qualified specialized primitive,
not evidence that 64 is optimal for relational joins or every GPU. Group data already remains resident while eligibility is refreshed in the
measured resident route. Extending that ownership across shared acquisition
and ordinary solver calls is a new hypothesis; it may be more valuable than
changing the reduction tree. For general formula acquisition, transposing
worlds across lanes could expose parallelism, but the exact frozen-DAG
dependency order remains authoritative. These are separate kernels and should
have separate measurements.

### 8. Certified source filtering and normalization

The lowest-risk transformations add a filter already entailed by a rule's
positive body, simplify a fully evaluated total guard, or factor independent
witnesses under established premises. More ambitious backward filtering should
be an admitted plan with explicit semantic obligations. The 2026 static-filter
result is a substantial lead, but a rewrite that discards hidden atoms needs
a constructive inverse if the library promises complete original answer sets.

`#show` is an output view, not blanket authorization to change the model space.
Constraints, optimization costs, optimum ties and hidden-model multiplicity
can remain observable even when an atom is not displayed. `#defined` similarly
does not imply a closed finite extension for every input predicate. Magic-set
literature also states language and query conditions; its disjunctive results
are not a theorem for arbitrary supported programs.
[Alviano, 2010](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ICLP.2010.226)

Preserve meaningful logical operations through lowering rather than
immediately recovering them from expanded Boolean structure. Typed multi-level
Datalog IR research supports the value of explicit binding and representation
boundaries. This aligns with themelios's program vocabulary and the existing
[library-first design](../design/library-first-20260907.md); it need not add a
new compiler framework dependency.
[Klopp, Erdweg and Pacak, 2024](https://www.pl.informatik.uni-mainz.de/files/2024/10/datalog-ir.pdf)

### 9. Cross-candidate incrementality and adaptive eagerness

Reuse within one monotone frozen epoch is simpler than reuse between
candidates. Changing a candidate can invalidate default-negation gates and
remove previously justified consequences. A cache keyed only by catalog atom
IDs is unsound: the same catalog can have different world memberships and
different frozen truth. Retraction or invalidation must cover every dependent
result.

After within-epoch delta evaluation succeeds, investigate conservative reuse
of candidate-independent rows, prepared indexes and expression plans. Reuse
of actual derived truth requires a stronger dependency certificate. Adaptive
eagerness could materialize a small repeatedly used region while retaining
lazy expansion elsewhere, but it must preserve a complete source schedule and
report the choice. This is a later opportunity, not a reason to delay a
bounded first optimization pass.

## 4. Semantic and formal obligations

Every experiment needs an explicit statement of what its input denotes and
what its successful result guarantees. At least six notions must remain
separate: set-valued relation facts, body-occurrence witnesses, complete
aggregate tuple keys, objective contributions, candidate occurrences, and
displayed models. Deduplication valid for one is not automatically valid for
the others.

For a layout transformation, the basic law is reconstruction of the original
logical rows, including required occurrences and stable identity. For a
binding plan, the law is coverage and soundness of substitutions relative to
the admitted source and snapshot. For a formula transformation, both truth
under a candidate and truth under its reduct must agree in the promised
contexts. None of these laws alone proves all the others.

The native aggregate formulation evaluates the aggregate condition on active
tuple sets in the original candidate and the frozen interpretation. The
frozen aggregate verdict conjoins the original guard result with the guard
result over frozen eligibility; the latter alone is insufficient. This follows
the applicable Ferraris characterization. Native evaluation can replace
expanded traversal only after the executable
bridge covers the actual group representation.
[Ferraris, 2005, proposition 7](https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf)

| Obligation | Existing Lean foundation | Required extension or bridge |
| --- | --- | --- |
| Binding columns preserve values and located faults | [ScalarArithmetic](../../proofs/Zetesis/ScalarArithmetic.lean): `operation_agreement`, `first_failure`; [JoinFrames](../../proofs/Zetesis/JoinFrames.lean): `prepared_prefix_exact` | A row-wise lift from finite scalar plans to tiles, plus the actual storage/decoder refinement and ordered fault commitment. |
| Delta schedules cover all consequences | [LazyRounds](../../proofs/Zetesis/LazyRounds.lean): `union_scan_covers_world`, `completed_round_exact`, `chunk_concatenation` | A body-occurrence delta decomposition, bootstrap law and schedule/closure bridge. Existing snapshot coverage is not already a semi-naive theorem. |
| World filtering cannot invent or omit a witness | [WorldMasks](../../proofs/Zetesis/WorldMasks.lean): `masked_scan_covers_world`, `cross_world_union_has_no_common_witness`, `stale_membership_can_omit_current_truth` | Preservation through tiled layouts, updated epochs and device projection; retain the stale-membership counterexample. |
| Join planning and factorized acquisition preserve witnesses | [JoinFrames](../../proofs/Zetesis/JoinFrames.lean); [RuleFactorization](../../proofs/Zetesis/RuleFactorization.lean): `original_factorization`, `frozen_factorization`, `stable_in_context` | A generic finite binding-relation law for index lookup, permutation and semijoin; connect it to the concrete matchers. |
| Tuple grouping preserves aggregate activity | [AggregateAssignment](../../proofs/Zetesis/AggregateAssignment.lean): `coalesced_active_tuples`; [AggregateDependencies](../../proofs/Zetesis/AggregateDependencies.lean): `covered_extension`, `frozen_rows` | Complete-key and condition-OR refinement, scoped predecessor bindings, shared acquisition identity. |
| Native reductions preserve original and frozen truth | [AggregateReduct](../../proofs/Zetesis/AggregateReduct.lean): `direct_reduct`, `evaluator_refinement`; [AggregateRanges](../../proofs/Zetesis/AggregateRanges.lean): `parallel_reduction_range` | Ordinary solver integration, eligibility acquisition and actual CPU/WGSL decoding refinement. |
| Source guard replacement is legitimate | [GroundGuards](../../proofs/Zetesis/GroundGuards.lean): `evaluated_guard_equivalent`, `replacement_in_formula_context` | A checked-admission premise showing the guard is fully evaluated and total; a speculative prefix is not covered. |
| Interruption and restrictions cannot certify acceptance | [CertifiedExecution](../../proofs/Zetesis/CertifiedExecution.lean): `interruption_cannot_accept`, `restricted_result_original` | New operators must return typed incomplete/failure outcomes and preserve accounting through batching and callbacks. |

Source arithmetic is currently checked `i32`. CPU aggregate sums use checked
`i128`; the admitted GPU numeric reduction uses bounded `i32` with a signed
envelope that bounds intermediate partial sums. A final total that fits is
insufficient to justify parallel reassociation. The existing
`bounded_total_is_insufficient` counterexample makes this explicit. Source
reassociation needs its own operation-order law; aggregate reduction's proof
must not be borrowed for arbitrary source expressions.

Ordered terms and extrema need equal care. Hash order, symbol IDs and storage
order are not automatically ASP term order. Empty extrema and `#inf`/`#sup`
are logical values, not numeric sentinels. The documented endpoint refusals
and unresolved recursive-extrema discrepancies remain separate from a
physical plan's numeric admission. See
[numeric semantics](../design/numeric-semantics.md). An optimization must not
silently change these boundaries or attribute an internal refusal to themelios.

Resource budgets are part of the executable contract. A new algorithm can
legitimately count different physical work, so identical numeric work limits
need not stop at identical candidates. What must survive is truthful partial
coverage, checked allocation, explicit stop reasons and no acceptance from an
unfinished computation. Trace-equivalence tests should use sufficiently high
budgets; exact-limit tests separately exercise the new accounting boundaries.

The proof library should add named semantic lemmas and short composition
proofs before implementation-specific refinements. Existing Lean laws verify
their mathematical models. They do not establish a complete refinement of
Rust, WGSL, drivers, source admission and I/O, and the solver should not be
described as fully formally verified on their strength.

## 5. Hardware plans

| Platform | Credible first implementation | Conditions and limits |
| --- | --- | --- |
| Scalar Rust | Bounded columns, prepared index plans, delta scheduling and shared eligibility | Essential reference and often best for tiny/irregular inputs. Measure changes against the current optimized scalar path. |
| Rayon CPU | Independent tiles, candidates or rule work with local output buffers and a checked merge | Avoid shared hot maps and nested pool oversubscription. Preserve ordering where promised. Parallelism can raise peak memory through simultaneous scratch buffers. |
| Metal through wgpu | Resident groups or relation tiles with several operators per submission | Apple unified memory can reduce physical transfer barriers, but allocation, packing, synchronization and readback remain costs. wgpu does not promise zero-copy access to Rust collections. |
| Vulkan through wgpu | The same admitted portable kernels, separately qualified on the available Radeon 780M | Record actual adapter features and driver versions. Large system RAM is not proof that every device buffer size or shader feature is supported. |
| NVIDIA through wgpu | Large batches and resident relational operations through the available native backend | CUDA papers motivate algorithms, not an automatic CUDA path in zetesis. Unknown DGX hardware must remain unspecified until observed. |

The project pins [wgpu 30.0.1](https://docs.rs/wgpu/30.0.1/wgpu/) and
[Rayon 1.12](https://docs.rs/rayon/1.12.0/rayon/). Adapter capabilities,
workgroup limits, shader integer operations and storage-buffer sizes determine
admission. Apple documents shared/private resource choices and CPU/GPU
synchronization even on unified-memory hardware.
[Apple Metal resource storage](https://developer.apple.com/documentation/metal/choosing-a-resource-storage-mode-for-apple-gpus?changes=_2&language=objc)

For an offload decision, measure the whole expression
`prepare + pack + upload + dispatch + compute + readback + decode + residual`.
Compare cold initialization and steady reuse separately. Fusing operators can
remove intermediate transfers; it can also increase live state and reduce
occupancy. Transposing a layout is justified only when subsequent reuse repays
its conversion and extra storage. Device-resident logical bytes and process
RSS answer different questions.

Multi-GPU grounding is a later problem. Published multi-node/multi-GPU Datalog
uses explicit partitioning and exchange, including MPI/CUDA communication.
That capability does not emerge automatically from a wgpu adapter selection.
Independent candidate partitioning appears a simpler first zetesis experiment
than distributed joins, but is an inference awaiting workload evidence.
[Shovon et al., ICS 2025](https://hpcrl.github.io/ICS2025-webpage/program/Proceedings_ICS25/ics25-71.pdf)

## 6. Experiment protocol

### Comparable semantic and physical boundaries

Freeze the original source, constants, includes, themelios pin, solver build,
compiler flags, feature set and device configuration. Use the in-repository
corpus without interpreting elenctic annotations. Compare all six queens
encodings across a declared range of `N`, SEND, shortest-path variants,
traveling-salesman and task-allocation families. Add generated joins that vary
selectivity, skew, arity, repeated variables, recursion depth and tuple-key
duplication independently. A small corpus alone cannot expose the intended
asymptotic differences.

Use three boundaries for each experiment. First, feed identical admitted
relations/bindings/groups into alternative physical operators. Second, measure
the owning grounding or source-scan phase including plan construction and
conversion. Third, measure full solves with the same enumeration, optimization
and output policy. A faster kernel that adds more acquisition or increases
search work has not established a solver improvement.

The comparison matrix should report eager CPU, lazy CPU, eager Metal and lazy
Metal wherever admitted, alongside clingo as an external semantic/performance
reference. Record refusals, timeouts and unavailable adapters explicitly;
exclude none silently from a favorable mean. Parsing/admission time, grounding
time, source scanning during solving, candidate generation, propagation,
original/frozen checks, objective work and I/O should be distinguishable.
Lazy execution has no single complete up-front grounding interval, so also
report cumulative source work and peak retained source state.

### Required negative controls

| Mechanism | Controls that can falsify the claimed benefit or safety |
| --- | --- |
| Numeric columns | No-arithmetic rules, tiny batches, mostly surviving rows, mixed symbols, structured terms, checked overflow, zero divisors and undefined prefixes with no completed relational extension. |
| Delta schedules | No recursion, very shallow closure, almost-all-new dense rounds, repeated predicate occurrences, empty-body rules, constraints becoming relevant late and multiple candidates sharing rows. |
| Index plans | Tiny relations, rapidly growing snapshots, unused columns, skewed heavy hitters, changing bound prefixes and patterns with no bound argument. |
| Shared eligibility | Disjoint dependencies, tiny groups, duplicate complete keys, equal weights with different keys, original/frozen disagreement and changed theory identities. |
| Multiway/semijoin | A selective chain that already streams well, cyclic joins with no output, dense unavoidable output, disconnected witnesses and cases already handled by factorization. |
| GPU layouts | One candidate, irregular tails, repeated resizing, transfer-dominated groups, output-count overflow, exhausted work/buffer limits, decode failure and interrupted source callbacks. |

Correctness checks compare complete native atom sets, selected model
occurrences, displayed terms, costs, priority presence and optimum ties as
applicable. Do not compare only a model count or one displayed answer. Operator
tests should use independent finite enumeration and property generation, then
clingo regressions at the source boundary. Stable-model parity does not excuse
changed source diagnostics or a false complete result after a resource stop.

### Timing and memory reporting

Use frozen old/new binaries and alternating or randomized paired order on an
otherwise quiet machine. Warm-up and measured repetitions must be declared;
cold startup belongs in a separate result. Retain all samples, paired ratios,
medians and variability. Report improvements by workload family before an
aggregate figure, and avoid pooling different adapters, builds or execution
policies. A new specialization needs a matched reference path, not only a
comparison with a historically slower build.

Record work counters that test the mechanism: relation/index bytes, index
construction work, posting lengths, matched/rejected prefixes, expression
evaluations, materialized terms, delta sizes, source rounds, group acquisition,
candidate counts, decisions and residual checks. Counters are explanatory,
not interchangeable units of time. The source-count experiment shows why
fewer decisions alone are inadequate.

For memory, separate logical payload, allocated capacity, allocator/process
RSS or high-water RSS, and device-buffer allocations. Capacity reuse can cut
allocation traffic without lowering RSS; a packed representation can reduce
payload while temporary conversion raises the peak. On unified memory,
process and GPU reports can refer to overlapping physical pages and should
not be added without understanding the measurement definitions. Include the
metric's collection method and platform in every report.

Rust tooling should emit machine-readable records with source/build hashes,
configuration, success/refusal/stop status and accounting completeness. Keep
raw artifacts and the procedure needed to regenerate them, including negative
results. This provides externally reviewable evidence without requiring a
Python analysis dependency or a new release process. Use the existing
proptest, Criterion, coverage, rustfmt, pedantic Clippy and Lean gates;
performance conclusions require controlled measurements beyond Criterion
correctness checks.

## 7. Bounded next experiments

These are the first three recommendations after intended-language closure.
They should be independently reviewable and may be declined independently.

1. **Binding columns, CPU first.** Keep the binding stream and formula output
   unchanged. Compare the current scalar evaluator with bounded numeric and
   comparison columns, then Rayon tiles. Instrument term construction and
   expression work as well as admission time and peak storage. Require gains
   beyond SEND and retain the no-arithmetic controls. If conversion erases the
   benefit, keep the evidence and do not route ordinary solves through it.
2. **Delta closure in one frozen lazy epoch.** Prove the occurrence schedule,
   implement one relational profile and compare it with complete rescanning.
   Separate scan reduction from index-maintenance changes. Use long sparse
   chains and dense/no-recursion controls, then the admitted corpus. No
   cross-candidate truth reuse belongs in this first implementation.
3. **Economical indexes and binding-aware order.** Instrument search patterns
   and evaluate one bounded planner against the existing shortest-posting and
   prefix-window baselines. Measure preparation, updates, probes and retained
   bytes. If the language tranche's profiles show aggregate eligibility
   dominating instead, substitute shared acquisition as the third experiment
   rather than running both unboundedly.

**First optimization pass:** complete these limited host-side experiments and
their semantic bridges, integrating only independently useful results. A
negative result is a completed experiment. Avoid simultaneously changing
language admission, source arithmetic width, candidate strategy and layout;
otherwise neither correctness differences nor timing differences are
attributable.

**Second optimization pass:** select one demonstrated substantial operator or
operator chain for physical Metal execution or improved residency. Candidate
examples are column filtering followed by compaction, shared acquisition
followed by native aggregate reduction, or delta-driven source batches. Include
ordinary solve integration and complete conversion/residual costs. If the
first pass supplies no sufficiently large repeatable operator, improve the
dominant measured CPU path instead of forcing a GPU claim. Vulkan and NVIDIA
qualification follow the same contract when those machines are available.

Language closure can expose useful common concepts before these experiments:
complete logical values and binding scopes, aggregate tuple activity separate
from head permission, ordered dependency producers and objective priority
presence. Capturing these concepts coherently is worthwhile now. Building
multiple speculative physical backends while they are still changing would
make both review and attribution harder.

## 8. Deferred work and decision boundaries

Do not undertake a wholesale CUDA-style Datalog engine rewrite, general
distributed grounding, or a new storage engine before bounded experiments
identify a bottleneck those designs address. Multiway joins and semijoin
reduction remain available when a concrete rule family warrants them; they
are not prerequisites for the first pass. Neuromorphic execution remains a
separate feasibility track rather than a reason to compromise checked
source semantics.

Defer global equality saturation, unqualified ngo pass adoption and
`#show`-driven model deletion. Their optimization freedom exceeds the current
equivalence/reconstruction bridge. Likewise, specialized recursive aggregate
algorithms require semantic premises rather than recognition of a predicate
name or a benchmark domain. Arithmetic width expansion to `i64` is a separate
language and proof change, although typed numeric plans would make a later
implementation cleaner.

Finally, do not infer a global subset-pruning rule from acceptance of one
answer set in the supported choice/aggregate language. Safe feedback needs
its actual semantic premises and a completeness proof. The permanent
foundation is the reduct: class-specific checks, native aggregates and source
plans may avoid work only through a justified equivalence or restriction.
The immediate opportunity is to expose and execute those meaningful
operations efficiently while keeping the original program and its answer
sets authoritative.

## Sources

Repository evidence and implementation anchors are linked at the associated
claims above. The research references below are primary publications, author
copies, project documentation or explicitly identified preprints. Living
documentation was consulted on September 8, 2026.

1. Yihao Sun, Sidharth Kumar, Thomas Gilray and Kristopher Micinski.
   [*Column-Oriented Datalog on the GPU*](https://arxiv.org/abs/2501.13051).
   January 22, 2025 preprint; AAAI 2025 work. Column storage and late materialization.
2. Yihao Sun, Ahmedur Rahman Shovon, Thomas Gilray, Sidharth Kumar and
   Kristopher Micinski.
   [*Optimizing Datalog for the GPU*](https://arshovon.com/publications/ASPLOS_2025.pdf).
   ASPLOS 2025, DOI 10.1145/3669940.3707274. GPUlog storage and relational execution.
3. Peter Boncz, Marcin Zukowski and Niels Nes.
   [*MonetDB/X100: Hyper-Pipelining Query Execution*](https://www.cidrdb.org/cidr2005/papers/P19.pdf).
   CIDR 2005. Bounded vector execution.
4. Timo Kersten, Viktor Leis, Alfons Kemper, Thomas Neumann, Andrew Pavlo and
   Peter Boncz.
   [*Everything You Always Wanted to Know About Compiled and Vectorized Queries But Were Afraid to Ask*](https://ir.cwi.nl/pub/28470/28470.pdf).
   PVLDB 11(13), 2018, DOI 10.14778/3275366.3275370. Matched execution-model comparison.
5. Jiaxin Lu and Sidharth Kumar.
   [*Accelerating Iterative Relational Algebra Operations with WebGPU*](https://www-new.evl.uic.edu/documents/jiaxin.pdf).
   GCASR 2025 poster. Portable GPU implementation and negative timing evidence.
6. Pavle Subotić, Herbert Jordan, Lijun Chang, Alan Fekete and Bernhard Scholz.
   [*Automatic Index Selection for Large-Scale Datalog Computation*](https://psubotic.github.io/papers/p141-subotic.pdf).
   PVLDB 12(2), 2018, DOI 10.14778/3282495.3282500. Search-pattern index coverage.
7. Samuel Arch, Xiaowen Hu, David Zhao, Pavle Subotić and Bernhard Scholz.
   [*Building a Join Optimizer for Soufflé*](https://souffle-lang.github.io/pdf/lopstr2022.pdf).
   LOPSTR 2022. Join-order planning with profile information.
8. Hung Q. Ngo, Ely Porat, Christopher Ré and Atri Rudra.
   [*Worst-case Optimal Join Algorithms*](https://arxiv.org/abs/1203.1952).
   2012 preprint, later journal version. Conjunctive join worst-case bounds.
9. Todd L. Veldhuizen.
   [*Leapfrog Triejoin: A Worst-Case Optimal Join Algorithm*](https://arxiv.org/pdf/1210.0481).
   2012 preprint; ICDT 2014. Ordered multiway join traversal.
10. Yisu Remy Wang, Max Willsey and Dan Suciu.
    [*Free Join: Unifying Worst-Case Optimal and Traditional Joins*](https://www.mwillsey.com/papers/freejoin).
    SIGMOD/PACMMOD 2023, DOI 10.1145/3589295. A unified physical join framework.
11. Yihao Sun, Kunting Qi, Thomas Gilray, Sidharth Kumar and Kristopher Micinski.
    [*Scaling Worst-Case Optimal Datalog to GPUs*](https://arxiv.org/html/2604.20073v2).
    April 23, 2026 preprint, version 2. SRDatalog layouts, incremental processing and skew.
12. Dan Olteanu and Jakub Závodný.
    [*Factorised Representations of Query Results: Size Bounds and Readability*](https://www.cs.ox.ac.uk/dan.olteanu/papers/oz-icdt12.pdf).
    ICDT 2012. Factorized witness representation.
13. Frank McSherry, Derek G. Murray, Rebecca Isaacs and Michael Isard.
    [*Differential Dataflow*](https://www.cidrdb.org/cidr2013/Papers/CIDR13_Paper111.pdf).
    CIDR 2013. Incremental updates and nested iteration.
14. Richard Taupe, Antonius Weinzierl and Gerhard Friedrich.
    [*Degrees of Laziness in Grounding: Effects of Lazy-Grounding Strategies on ASP Solving*](https://arxiv.org/abs/1903.12510).
    2019. Grounding/search tradeoffs.
15. Jori Bomanson, Tomi Janhunen and Antonius Weinzierl.
    [*Enhancing Lazy Grounding with Lazy Normalization in Answer-Set Programming*](https://repositum.tuwien.ac.at/handle/20.500.12708/58118).
    AAAI 2019, DOI 10.1609/aaai.v33i01.33012694. Lazy aggregate normalization.
16. Philipp Hanisch and Markus Krötzsch.
    [*Rule Rewriting Revisited: A Fresh Look at Static Filtering for Datalog and ASP*](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ICDT.2026.5).
    ICDT, March 18, 2026, DOI 10.4230/LIPIcs.ICDT.2026.5.
    [Full proof version](https://arxiv.org/html/2601.05108v1).
    Static filtering and its restricted stable-model correspondence.
17. Potassco. [*ngo*](https://potassco.org/ngo/). Living project documentation.
    Syntactic ASP optimization candidates.
18. Vladimir Lifschitz, David Pearce and Agustín Valverde.
    [*Strongly Equivalent Logic Programs*](https://www.cs.utexas.edu/~vl/papers/ht.pdf).
    ACM TOCL 2(4), 2001. Replacement under arbitrary program contexts.
19. Patrick Cousot and Radhia Cousot.
    [*Abstract Interpretation: A Unified Lattice Model for Static Analysis of Programs by Construction or Approximation of Fixpoints*](https://www.di.ens.fr/~cousot/COUSOTpapers/POPL77.shtml).
    POPL 1977. Sound approximate domains.
20. Mario Alviano.
    [*Dynamic Magic Sets for Disjunctive Datalog Programs*](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ICLP.2010.226).
    ICLP 2010 technical communications, DOI 10.4230/LIPIcs.ICLP.2010.226.
    Query-directed rewriting under a restricted language.
21. David Klopp, Sebastian Erdweg and André Pacak.
    [*A Typed Multi-level Datalog IR and Its Compiler Framework*](https://www.pl.informatik.uni-mainz.de/files/2024/10/datalog-ir.pdf).
    OOPSLA 2024, DOI 10.1145/3689767. Binding-aware progressive lowering.
22. Paolo Ferraris.
    [*Answer Sets for Propositional Theories*](https://www.cs.utexas.edu/~ai-lab/pubs/proptheories.pdf).
    2005. Propositional reduct and aggregate characterization.
23. Ahmedur Rahman Shovon, Yihao Sun, Kristopher Micinski, Thomas Gilray and
    Sidharth Kumar.
    [*Multi-Node Multi-GPU Datalog*](https://hpcrl.github.io/ICS2025-webpage/program/Proceedings_ICS25/ics25-71.pdf).
    ICS 2025, DOI 10.1145/3721145.3730431. Explicit distributed GPU data movement.
24. LLVM/MLIR project.
    [*SparseTensor dialect*](https://mlir.llvm.org/docs/Dialects/SparseTensorOps/).
    Living documentation. Logical/physical sparse representation separation.
25. W3C GPU for the Web Working Group.
    [*WebGPU Shading Language*](https://www.w3.org/TR/2026/CRD-WGSL-20260831/).
    Candidate Recommendation Draft, August 31, 2026. Shader and synchronization contracts.
26. gfx-rs contributors. [*wgpu 30.0.1*](https://docs.rs/wgpu/30.0.1/wgpu/).
    Versioned API documentation. Portable backend and device abstractions.
27. Rayon contributors. [*Rayon 1.12.0*](https://docs.rs/rayon/1.12.0/rayon/).
    Versioned API documentation. CPU parallel execution.
28. Apple.
    [*Choosing a resource storage mode for Apple GPUs*](https://developer.apple.com/documentation/metal/choosing-a-resource-storage-mode-for-apple-gpus?changes=_2&language=objc).
    Living Metal documentation. Unified-memory resource ownership and synchronization.
29. Mihalis Yannakakis.
    [*Algorithms for Acyclic Database Schemes*](https://www.researchgate.net/profile/Mihalis-Yannakakis/publication/200034379_Algorithms_for_Acyclic_Database_Schemes/links/5745c2a708ae9f741b430b62/Algorithms-for-Acyclic-Database-Schemes.pdf).
    VLDB 1981, pp. 82–94, author-hosted copy. Acyclic joins and semijoin reduction.
