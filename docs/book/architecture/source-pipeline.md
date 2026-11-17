# Source preparation and grounding modules

<!-- A dated record: its commands keep the spellings of the binaries it records. -->

`zetesis-themelios` has two materialization boundaries. Relational admission
returns templates for lazy or eager normal-rule execution. Formula preparation
returns scoped instructions; grounding consumes them to construct a finite
Ferraris theory. Automatic CPU solving can instead certify and directly evaluate
a complete stratified normal source, retaining its original program and canonical
interpretation without a formula graph. Such a source may be
[recursive through certain components](#direct-evaluation-of-stratified-recursion). None of these preparation paths searches
candidate regions.

This chapter maps the implementation for readers extending the source language.
The [source API guide](../rust/source.md) describes the public entry points;
[grounding](grounding.md) explains their mathematical contracts.

Closed-range facts use one checked product cursor in formula admission. It retains interval endpoints and scalar alternatives, then emits one
head at a time in source order. Formula admission consumes that head directly. Complete-family
validation and logical expansion limits precede emission. This removes temporary
range columns and per-statement template vectors, without changing duplicates,
provenance or arithmetic diagnostics.

## Direct evaluation of stratified recursion

Datalog-shaped programs (closures, joins and arithmetic filters, with negation
over completed lower strata) take the direct route. A dependency component is
*certain* when each atom its rules negate lies in a strictly lower certain
component, and each atom they read positively lies in it or in a lower certain
component. Its bodies are flat atoms and comparisons, so its predicates are
certain in the sense of
[completed possible support](grounding.md#completed-possible-support): every
negation it reads is decided, support completion passes over the rows a
decided negation falsifies, and the remaining rules form a definite program
whose least model is exactly their completed support
([`CertainAtoms.derived_stable`](../lean/theorems.md)). A certain component may
therefore be recursive. In a complete normal source without negative or
aggregate cycles every component is certain, by induction along the ranks.

The plan ranks components as before. A non-recursive component that negates
is folded once against the completed lower activity, as an acyclic one is. A
*saturated* rank (recursive, or free of default negation) re-joins each rule
once over completed support, which also repeats the original key validation,
skips the rows a decided negation falsifies, as support completion did, and
stages every remaining row's head as required without folding its body;
same-rank atoms are never read before they are published. Before that, it
checks that support completion decided every default negation of the rule; a
disagreement between the plan and support refuses the evaluation rather than
publishing. Its heads are collected in a deferred source batch and merged into
the local selection when the rank ends, by one sorted linear merge rather than
one shifting insertion per atom. A negative or aggregate cycle declines the
class, retaining the ordinary grounding door and every accepted charge.

A rule may also hold an assignment aggregate, such as
`c(X,N) :- n(X), N = #count{ Y : e(X,Y) }.`, whose elements read only positive
atoms of strictly lower certain components and comparisons. Every tuple such
an element yields over completed support is required, so the assignment's
carrier is the single exact value that support completion proposes, and the
head predicate stays certain. Its rank is saturated: the rows support offers
are published without a fold, which has no aggregate reading. Evaluation
again first checks that support completion listed every element as required.
Any other aggregate, or an element with negation or a nested literal, keeps
the ordinary door.

The resulting rank structure is a stratification in the sense of
[`StratifiedEvaluation`](../lean/theorems.md): positive dependencies stay
within a rank and frozen gates read lower ranks. Strict ranks use the one-pass
scan of `AcyclicEvaluation`; saturated ranks use the complete positive closure,
filtered by decided gates, that support completion has already computed.
Constraints and opposite classical signs then decide membership, as before.
Component recognition, the support fixed point and the Rust execution remain
separate correspondence obligations, as for the acyclic class.

## Follow the owned objects

```text
original source catalog → parsed and raised themelios program ┐
canonical Arc<Program> → bounded logical inspection           ┴
  → shared bounded normalization and scope analysis
  → prepared rules, objective/projection declarations and dependency plans
  → completed possible-positive support
  → complete bindings and original formula instances
  → validated theory + atom catalog + objectives + fixed projection domain + metadata
```

The arrows describe dependencies, not independent copies of every intermediate
object. An immutable original program owner survives normalization and grounding;
source input additionally retains its original catalog. Statement IDs and any
real parsed origins remain available after preparation.
Both the source-expansion budget and accepted formula-work charges continue
across preparation and grounding. Materialization resumes the retained accounting
under its configured ceilings.
An analysis projection records its own basis; its classifications do not prove
properties of source constructs that the projection erased.

Possible support uses one canonical source authority. A completed snapshot lends
immutable relations and column indexes to joins; it does not grant those atoms
truth in an answer set. Final grounding publishes a distinct dense occurrence map
over the same canonical prefix. The theory's universe and possible-support bound
remain different populations without separately interning their payload.
Objective queries borrow completed support and own bounded transient formula
scratch. Their atoms and roots cannot become program producers.
Projection declarations also borrow that support. They retain a fixed subset of
possible atoms for enumeration identity, without creating source producers or
program roots. This domain is distinct from the emitted theory's dense IDs.

`GroundingOptions` selects the positive-join execution strategy before formula
materialization. At the indexed fallback, the existing value posting is resolved
first. An empty or singleton posting keeps the ordinary matcher and delta selection.
For a wider candidate domain, a fully bound flat pattern borrows a complete typed
key and uses the relation's existing exact lookup. Its empty or singleton result
keeps the original row identity and current delta partition; it allocates no tuple
or new index. Both index operations belong to one logical join probe, with all
performed work charged. Partial patterns retain the original value posting. Existing structural, domain and arithmetic selections retain their
precedence, and all offered rows still pass the ordinary matcher and filters.
Explicit `Table` selection prepares reusable value-support masks
for flat positive patterns over completed eager support. Growth-round snapshots
and structural patterns keep indexed probes. This changes the source of row
positions, while the existing whole-row matcher, binding schedule and original
formula emission remain shared.

The completed query workspace borrows the authoritative relations and owns its
index cache. Each join depth owns a selected row mask that borrows the relation,
so later cache insertion does not invalidate an earlier selection. Indices are
keyed by signed predicate and canonical alias scope; domains are rebuilt from
current constants and bound values. Required body validation receives the same
positive witnesses. Table membership never replaces a source atom with truth.

The workspace charges cumulative preparation/query work and simultaneous named
snapshot, index, scratch and mask capacities to the existing source limits.
Refused capacity, allocation and work retain logical scope and any real source
coordinates; a failed table operation is not an indexed fallback or an empty program. These
objects are not a process-memory ceiling. Selecting the table strategy itself
launches neither Rayon grounding nor a GPU kernel. Canonical construction and
publication remain coordinator-owned; separately enabled row execution follows
the [ordered execution boundary](#ordered-row-execution-and-current-activity-reuse)
below. See the [table API and its preservation boundary](../rust/finite-tables.md).

## Ordered row execution and current-activity reuse

The experimental source executor belongs to one initial preparation, spanning
possible-support completion, formula emission and objective activation. Its
physical worker selection is separate from the solver's search-worker count;
the current private configuration is not a public grounding default. Retained
per-answer callbacks do not acquire this pool. The executor neither selects a
new source semantics nor changes reduct membership.

Eligible operations borrow the existing Join plan and immutable relation epoch.
They partition an admitted probe into contiguous blocks, use the original
matcher and scalar filters, and return ordered source witnesses. A fully bound
prefix may precede that probe only when its ordinary matcher proves a zero-or-one
row prefix without binding new variables. Workers still execute those prefix
reads. An expanding-generator check prevents creating staging work for a join
with no useful varying suffix; it does not establish profitability.

The coordinator consumes successful blocks in source order and retains canonical
term and atom construction, provenance, formula emission and objective-family
publication. Support execution preserves the first-new delta pivot and serial
publication barriers. An immutable published snapshot does not imply that the
pending-head prefix is constant: earlier committed rows can suppress later
support witnesses. Therefore fixed-stream list equality alone does not prove
that staging visits the same internal witness count as per-row serial emission.
The public requirement is preserved complete source meaning and output; the
actual performed work remains accounted.

Each consumer authenticates its own shape and current cursor state. The
source-activity producer consumer admits normal positive flat patterns with
comparison operands that are single constants or variables. It does not admit
general arithmetic by borrowing this rule. Completed table-query consumers,
retained selections, generated values, rich scopes or other unsupported states
keep their existing traversal when the descriptor cannot establish eligibility.
A syntactically eligible objective may select a different dependency-analysis
path; actual dispatch must be established at the final consumer.

One source-owned pool serves bounded tasks without nested pools. Each task has
its own budget and counters; all performed receipts, including those of failed
tasks, settle before outcome arbitration and ordered publication. Storage
reservations remain live while the coordinator consumes results. This can cause
earlier refusal under a partitioned allowance. A descriptor or pool that declines
before claiming the cursor can select the ordinary path; an admitted stopped
attempt cannot retry with a fresh allowance. Actual cancellation and typed
resource failures retain their existing owners. Reservations describe admitted
logical storage, not a measured process-memory peak. The authored source-buffer
byte ledger excludes Rayon's pool/registry allocations, OS thread stacks and
per-spawn heap jobs. These infrastructure costs belong in process-time and
memory observations; the ledger does not claim their exact allocation size or
recovery from an allocator abort. Whole-child RSS does not isolate those costs.

Source activity can avoid a comparison only for a binding whose actual Join or
authenticated replay already established that same ordinary comparison. Atom
activity and guards still read the current table. Generic model-query and
objective activity have no such permission merely because their fields exist.
Removing the redundant arithmetic also removes its work, intermediate-value
admissions and polling points; exact resource boundaries can consequently move.

During cyclic activity refinement, a producer may retain one completed immutable
row family over the same completed support. Its first pass records a row and
immediately runs the original consumer. Only complete exhaustion and successful
consumers seal the family; failure cannot publish a reusable partial stream.
Replay authenticates the source producer, relation epoch and canonical values,
keeps occurrence order and duplicates, and recomputes current Activity, bodies
and heads. It never caches a previous round's truth. An empty completed family
is valid. Retention storage starts lazily at the first eligible capture and stays
charged while live. The combined route tries sealed replay, then eligible cold
capture, then row workers, then the ordinary traversal; these savings overlap.

Objective scope validation also retains a smaller fact: only a successful full
scan proving no nested aggregate, conditional or projected-witness obligation
allows that immutable fragment's later bindings to skip another empty scan.
Rich scopes remain pending even if one binding has no descendants. Enclosing
row validation and independent scalar fields remain required. General objectives
have one such owner per scan; parallel numeric blocks each have their own.

The [ordered execution correspondence](../lean/correspondence.md#ordered-parallel-grounding-and-witness-replay)
and [comparison/row-reuse correspondence](../lean/correspondence.md#selected-comparisons-and-retained-producer-rows)
state the separate Lean premises and Rust obligations. These laws do not prove
thread safety, pool liveness, source matching, resource accounting or complete
frontend route selection. This implementation provides CPU row execution, not
device grounding or a general parallel component-wave scheduler.

### Measuring the development worker configuration

The current development build attempts a source-owned pool with 14 physical
workers and a logical task ceiling of 14. This setting is separate from the
solver's search-worker count: `--threads` does not select source workers. A
smaller admitted row interval can create fewer blocks. Pool initialization may
select the ordinary path, and the clean ordinary output does not report source
task counts or physical occupancy. A configured worker count therefore does not
establish observed concurrency.

Worker eligibility does not establish profitability. Much of a program may
remain outside the row certificate, and admitted workers do not take ownership
of canonical formula construction, current Activity or semantic round
publication. For example, a headless negative transitivity constraint and
arithmetic choice constraints remain outside the normal positive-flat emission
worker. A cyclic objective with double-negated conditions requires the general
objective path, rather than the lifted numeric scan. A dependent reachability
chain can repeatedly offer a narrow stream while later rounds still wait for
publication. These are distinct limits; more threads do not remove them.

A targeted source family exercises the lifted numeric class directly:

```asp
d(1..512).
e(1..512).
#minimize { 1@0,X,Y : d(X), e(Y) }.
```

Replacing both endpoints by `N` gives `N*N` distinct objective contributions,
with one complete answer and optimum `N*N`. The formula's fact domains grow
linearly, while complete objective evaluation grows with their product. The
positive-flat condition, constant priority/weight and independent variables
meet the numeric-row certificate without a source-eligibility query or cyclic
retention.

On 2026-11-16, a development screen on an Apple M4 Pro compared separately
frozen CPU-only builds with configured source workers 1, 4 and 14. Rust 1.97.1
built the release profile with fat LTO and one codegen unit; search was fixed at
14 workers, clingo 5.8.2 used one worker, and the native allowance was 24 GiB.
The maintained benchmark engine ran one warmup, three timed repetitions and
one separate whole-child memory reading per point, with a 120-second child
limit. Complete optimal families and costs were checked. Initial-grounding
medians, in milliseconds, were:

| N | Configured source 1 | Configured source 4 | Configured source 14 |
|---:|---:|---:|---:|
| 512 | 38.419 | 16.590 | 38.151 |
| 1,024 | 149.735 | 74.623 | 150.226 |
| 2,048 | 601.167 | 283.204 | 582.218 |

Four workers reduced this phase by about 2–2.3 times; fourteen did not retain
that benefit. At `N=2048`, total process medians were 3.406, 3.166 and 3.480
seconds: the best observed total reduction was about 7%, because later objective
scoring/retention still took roughly 2.8 seconds. Whole-child native peak RSS
was about 341, 346 and 343 MiB, respectively. Those peaks do not isolate pool,
retention or grounding allocations. All named grounding work/outcome records
and published Formula counts agreed across these settings; the observed gain
was in elapsed execution, not fewer counted bindings or arithmetic operations.

This is a narrow, ordered three-sample screen, not a general worker default.
The broader size controls exposed substantial serial-facing work, and the
reachability-chain control became slower with more configured source workers.
The cause of the numeric 4-versus-14 behavior remains unmeasured: actual source
occupancy, scheduling and contention were not recorded. Exact source admission
and resource failures remain results rather than reasons to increase limits.

Use the [maintained benchmark protocol](../reference/benchmarking.md) for
reproduction. Source-worker comparisons currently require separately built
copies differing only in the private `PHYSICAL_WORKERS` constant in
`crates/zetesis-themelios/src/formula_support/parallel_executor.rs`; keep the
logical task ceiling fixed at 14. From each source copy, build the CPU executable
with the same toolchain and profile, using a distinct target directory outside
any synchronized source tree:

```sh
cargo +1.97.1 build --release -p zetesis-cli --bin zetesis \
  --no-default-features --locked --offline
```

Prepare the three input files shown above, using endpoints 512, 1024 and 2048.
Declare each as an authored case in a version-2 `--workloads` manifest named
`numeric-objective.json`, with its actual byte hash, an optimal-family contract
of one model, and the corresponding costs 262144, 1048576 and 4194304. Keep the
manifest and source bytes identical across worker configurations. For each
frozen executable, substitute its path and a fresh report name in the same
canonical command; the following names are placeholders, not installed tools:

```sh
zetesis-bench run examples/correctness \
  --workloads numeric-objective.json --zetesis ./zetesis-source-1 \
  --clingo clingo --native-interface solve --backend cpu \
  --grounder auto --oracle auto --threads 14 --clingo-threads 1 \
  --warmups 1 --repetitions 3 --memory-runs 1 \
  --timeout-seconds 120 --campaign-seconds 7200 --memory 25769803776 \
  --sample-bytes 134217728 --answer-bytes 134217728 \
  --native-report-bytes 134217728 --native-atoms 4194304 \
  --native-value-nodes 8388608 --capture-bytes 2147483648 \
  --report-bytes 4294967296 --report source-1.json --json --color never
```

Preserve full family/cost qualification, timed ranges, actual grounding mode
and fresh reference measurements. Report initial grounding and later
objective/validation work separately: overlapping phase clocks are not an
additive wall-time decomposition. These dated measurements belong to their
frozen CPU builds, not later source revisions or a GPU configuration. They do
not replace full-corpus comparisons or choose a CPU/GPU production policy.

## Locate the operation

The names below are modules under
[`crates/zetesis-themelios/src`](https://github.com/GregoryGelfond/zetesis/tree/main/crates/zetesis-themelios/src).
An `_ir` suffix denotes source lowering or scope checks; a `_plan` suffix
denotes a checked schedule over existing instructions. Execution consumers retain
those instructions rather than interpreting the plan as new program semantics.

| Responsibility | Modules | Boundary |
| --- | --- | --- |
| Source catalog and includes | `bundle`, `bundle_admission` | Original files, paths, source identities and located failures |
| Diagnostic and strict-profile checks | `profile`, `diagnostic`, `source_diagnostics` | Typed refusals and source views; no recovered partial program |
| Relational templates | `compile`, `extended`, `expansion`, `fact_expansion` | Closed values, bounded constants/arithmetic/fact expansion and template admission |
| Fact blocks | `formula_facts`, `formula_ir::facts` | On the formula route, facts held in fact blocks, products and runs of admitted values, with their statements' evidence, standing in item order where rules with empty bodies would have stood |
| Classical negation | `coherence` | Distinct signed predicates and consistency constraints |
| Semantic value conversion | `structural_value`, `scalar_arithmetic`, `formula_value` | Typed closed values and checked arithmetic; no atom support |
| Formula preparation | `formula`, `formula::preparation`, `formula_ir` | Public owned preparation, scoped rule/objective IR and cumulative budgets |
| Canonical input inspection | `program_limits`, `formula_program_check` | Bounded borrowed traversal before relational compilation or formula normalization |
| Counted occurrences and alternatives | `formula_ir`, `formula_pool` | Canonical counted entries, bounded pool alternatives and expanded Boolean identities |
| Upstream analysis | `formula_analysis`, `formula_conditional_projection` | Bounded pool-free input and an explicit analysis basis |
| Value and pattern lowering | `formula_value_ir`, `formula_range_ir`, `formula_pattern_ir`, `formula_projection_ir` | Shared bounded value/range fold, structural captures and anonymous negative projection |
| Scalar bindings | `formula_binding_ir`, `formula_binding_guard`, `formula_binding_plan` | Safe producers, finite envelopes and dependency order; original guards remain |
| Aggregate bindings | `formula_aggregate_ir`, `formula_assignment_ir`, `formula_assignment_plan` | Element scopes, assignment applicability and required/produced inputs |
| Binding execution | `formula_binding_cursor`, `formula_assignment` | Bounded scalar/range and aggregate proposals over complete relational rows |
| Head lowering | `formula_head_ir`, `formula_choice_ir`, `formula_head_aggregate` | Signed head activity, independent permission and aggregate measure |
| Conditional lowering | `formula_conditional_ir`, `formula_consequent_ir`, `formula_conditional_head_ir` | Outer/local scopes and the admitted conditional-head profile |
| Complete joins and possible support | `formula_support`, `formula_pattern` | Transactional whole-tuple matching, snapshots, indexes and support completion |
| Positive-row query strategy | `grounding_options`, `formula_support::queries`, CPU `table` | Explicit completed-support table indices and owned row masks; shared matcher and cumulative source budgets |
| Formula emission | `formula_ground`, `formula_guard`, `formula_conditional` | Original implications, aggregate conditions and universal instances |
| Completed aggregate families | `formula_ground::families`, `formula_binding_cursor` | One builder-owned coordinate/registry workspace, shared completed carriers and checked ordered occurrence replay |
| Optional formula factoring | `formula_factor` | Independent body components over fixed heads, and shared continuations for equivalent existential witnesses; arithmetic components require full-carrier certification |
| Optional count certificates | `formula_count_plan` | Capture, derivation and emitted correspondence to the original theory |
| Source activity | `formula_source_activity` | Bounded absent/optional/required producer refinement over completed support, shared by objectives and projection; each fact row is a required producer |
| Original-model queries | `formula_source_activity::model_query` | Closed conditions over full original atom identities, separate from source activity and reduct formulas |
| Objective source preparation | `formula_weak`, `formula_ir::objective_scope`, `formula_objective_dependencies` | Scoped weak bodies and independently applicable objective precision plans |
| Objective materialization | `formula_ground::objectives`, `formula_ground::scoped_body` | Retained objective templates and bounded body validation without producer support |
| Projection declarations | `formula_project_ir`, `formula_ground::projection`, `metadata::projection` | Fixed complete projection domain; no change to original theory or displayed atoms |
| Observation and metadata | `metadata`, `observation` | Source directives and views of a supplied full model |
| Objective bounds | `objective_bound` | Optional score bounds with their own admission and work limits |
| Ordered row execution | `formula_support::parallel_executor`, `formula_support::row_blocks` | One initial preparation owner, authenticated ordered witness staging and settled task receipts; consumer-specific admission |
| Source-activity reuse | `formula_source_activity::parallel`, `formula_source_activity::retained` | Selected bindings and sealed immutable row families; current Activity and canonical consumers remain coordinator-owned |
| Objective scope absence | `formula_ground::arithmetic::Scopes` | Successful complete syntax absence, not a cached result for one binding |
| Measurement | `grounding_observer` | Injected phase/work observations, separate from semantic completion |
| Public composition | `lib`, `formula`, `formula_owner`, `program_site` | Source/logical admission, retained original program and explicit statement identity |

Nested modules follow their owning module's directory: for example,
`formula_ir::objective_scope` is stored in `formula_ir/objective_scope.rs`.
The owning module declaration determines Rust visibility. Filename proximity does
not grant access or establish dependency direction.

## Keep the obligations separate

A binding plan proves readiness of inputs, not the truth of an aggregate or a
negative literal. A possible-support row proves that grounding must consider an
atom, not that the atom belongs to a candidate. An objective eligibility carrier
retains possible contribution keys; its original-model query still decides
whether a contribution is active. Formula emission and final theory validation
preserve these distinctions before any reduct check begins.

Required body evaluation and contribution selection are separate operations.
Pruning a false comparison must not hide required arithmetic on a complete
positive binding. Computations introduced solely for a head have their own
scope: a discarded body row must not acquire a head value or emit an atom.
The existing expression evaluator and binding schedule serve these scopes; their
placement determines when evaluation is required.

`formula_ground::scoped_body` shares the original body-lowering operation between
objective query construction, source activity and validation of discarded rule rows. The caller
selects the applicable scratch ceilings and retains cumulative source work.
Discarding the resulting scratch does not discard errors or charged work.

The source-activity fold computes absent, optional or required truth on the
validated body. The unresolved dependency region refines one complete previous
table in simultaneous rounds; this is not a separate evaluator per strongly
connected component. A changing round resolves an optional entry; it never
reclassifies an already known entry. The finite optional-entry count bounds the
rounds. This abstraction does not decide correlations among optional atoms, and
it does not replace original-model query evaluation or reduct satisfaction.

Rich producer activity keeps scoped formula construction within one producer
pass. Bindings that select different values from the same aggregate family reuse
that family's native rows and equality roots. The existing aggregate identity
and outer-binding key still distinguish different families; other body literals
are lowered for their own bindings. Formula and atom coordinates never leave
their owning scratch builder or become part of the original theory.

Within an unchanged activity table, completed node readings can be reused as
the scratch graph grows. Each native threshold row reduces its required and
possible contributions once, then its bounds answer the row's individual cuts.
The ordinary Boolean composition remains responsible for equality and other
combined conditions. Reuse ends before a new activity table is published;
direct publication requires fresh readings. This preserves the abstraction's
independent treatment of optional atoms. Existing work and storage limits cover
the retained scratch and actual operations, including interruption and refusal.

Both scratch and retained formula construction begin with the same canonical
constants: `FALSUM` represents bottom, and `VERUM` represents bottom implying
bottom. `Builder::initialize` admits these nodes through the ordinary checked
node operation. Boolean simplification uses these named identities. If either
admission fails, the scratch caller restores its cumulative counter owner before
returning the error.

Compiler-internal assignment tests live under `formula_ir` because they construct
both valid and invalid private IR. Public source tests cannot reach states that
scope checking rejects first. These tests complement source admission and
answer-set tests; they do not replace them or justify making private constructors
public.

When adding a construct, identify its source scope, readiness rule, complete-row
validation, possible-support contribution and original formula separately.
Then identify its original statement identity, any source locations, bounded
scratch and retained output. A change in one stage must not silently transfer
an obligation to the next.
