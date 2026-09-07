# Implementation map

This document maps the broader [design](design/zetesis.md) to the current code.
The design describes the experiment's target architecture; this file identifies
the implemented subset and its remaining qualification work. Full clingo language
and solving semantics, excluding theory terms and Python/Lua scripting, is the target. The
original non-clingcon kr-domains cases are a required initial corpus, specified
in the [corpus contract](verification/kr-domains-compatibility.md). The
[Ferraris/clingo extension](design/ferraris.md) supplies the semantic basis for the
implemented finite formula profile.
S0 is an initial slice and does not satisfy that full target.

For a first checkout, use the [development guide](development.md). The
[verification index](verification/status.md) distinguishes current source changes
from the last fully qualified binary. The reduct remains the foundation across
all source profiles and execution plans.

## Workspace

| Crate | Responsibility |
| --- | --- |
| `zetesis-core` | Typed relational values, safe templates, program identity, sparse seeds, fallible carrier iteration and explicit static lowering |
| `zetesis-domain` | Conservative signed predicate-position domains over the exact borrowed themelios `Program`, with explicit unknown results and no solver integration |
| `zetesis-themelios` | Explicit S0, scalar and formula library contracts; automatic CLI admission, bounded original include graphs, located refusals and normalized templates using pinned themelios |
| `zetesis-cpu` | Candidate-directed reduct joins, exact closure, packed static baseline, incremental candidate stream and owned Rayon pool |
| `zetesis-ferraris` | Finite formula DAG transforms, exhaustive reduct checking, checked tight support plans and normal-rule formula translation |
| `zetesis-objective` | Lifted positive objective joins, global tuple coalescing, checked costs and explicit per-model limits |
| `zetesis-sat` | Native bounded candidate and frozen-reduct countermodel search with independent witness validation |
| `zetesis-wgpu` | Bounded static integer oracle and general Ferraris propagation primitive, native adapter selection, resident buffers and GPU result transport |
| `zetesis-cli` | Prepared-input sessions, typed solve configuration/outcomes, bounded human/JSON views and process adaptation |
| `zetesis-telemetry` | Optional typed host stage measurements, independent of rendered diagnostics |
| `zetesis-experiments` | Standalone static and general-formula membership qualifications, explicit CPU residual measurements and Criterion regression benchmarks |
| `zetesis-validation` | External-oracle corpus comparisons; separate from every solver execution path |

Public types document their contracts and error conditions. Libraries do not
invoke an external ASP solver. The optional clingo campaign uses one only as an
independent test oracle.

## Signed identity and source coherence

All three native admission profiles retain predicate sign separately from name
and arity: `p(1)` and `-p(1)` are different atoms, while numeric minus in `p(-1)`
is part of a scalar value. A positive-body occurrence of either sign can bind a
variable. Source admission supplies coherence constraints for opposite complete
atoms; the lower-level core only preserves identities and does not add source
coherence automatically. Constraints forbid both signs together, supply no
support, and do not require either sign to be true.

Relational admission appends bounded, located integrity templates for opposite
signatures. Formula admission checks opposite identities in its completed atom
catalog and appends bounded constraint roots. Original source evidence from
both signs is retained, including across files. Signed objective and observation
conditions match the full predicate identity; signed `#defined` and `#show`
select the corresponding polarity. Hidden models and all optimal ties remain
distinct, including equal displayed-symbol records.

Under the checked clingo 5.8.2 source boundary, unsigned `not p(_)` and
`not not p(_)` use the admitted existential projection. Signed `not -p(_)` and
`not not -p(_)` are unsafe and refused; ordinary positive-body `-p(_)` is a valid
binder. Closed signed constructors and tuples now have bounded logical identity
through the [structural value carrier](design/closed-structural-values.md).
Formula admission also supports finite construction from independently bound
inputs, evaluated safe negative arguments and positive tuple/function-pattern
extraction. Function patterns retain name, sign and arity, including nested
constructors, repeated variables and independently anonymous positions. Matching
uses staged binding deltas so an unsuccessful row cannot modify the caller's
environment. Whole captures retain the original body atom in the formula. The
[source API guide](../crates/zetesis-themelios/README.md) gives the exact scope.
The [function-pattern record](verification/function-patterns-20260907/README.md)
retains the constructor and transactional-matching evidence. Evaluated positive
positions, such as `p(X+1)` or `p(f(X,X+1))`, compare captured complete values
with the existing expression plan after independent inputs are ready. Ordinary
positions in the same atom can supply those inputs. These checks cannot infer
inverse bindings or supply support; the [positive-argument record](verification/positive-arguments-20260907/README.md)
states the source, scope, failure and resource contracts.
See the [strong-negation record](verification/strong-negation-20260906/README.md)
for independent full-model/reduct tests and retained reference diagnostics.

The Lean [signed identity/coherence laws](../proofs/Zetesis/StrongNegation.lean)
cover injective renaming, frozen reducts, stable-model correspondence and
constraint-only coherence under explicit pair coverage. They do not verify the
source compiler, carrier construction, Rust matching, resource limits or any
backend.

## Certified support and parallel reduct completion

`TightPlan` inspects every root of an immutable completed Ferraris theory and
extracts a restricted normal/atomic-choice producer grammar. It checks strict
positive ranks over a linear-size formula/atom dependency graph. Source analysis
is advisory: neither a source category label nor a partially materialized lazy
theory supplies this certificate. Constraints and candidate-frozen negations
retain their original truth checks.

For a certified theory, original satisfaction together with support of every
present atom proves stable membership. `TightPlan::check` therefore returns
`Stable` without an inner subset search. A missing producer returns `Residual`,
leaving witness-based rejection to the general checker. Unrecognized syntax,
cycles, incomplete work and exhausted plan limits cannot produce a certificate.
The Rust API and complete corpus experiment exercise this composition. Ordinary
CPU formula solving selects it under `--oracle auto` when the completed theory
receives the required certificate; explicit countermodel selection keeps the
general reduct path. The [Lean laws](../proofs/Zetesis/TightPlans.lean)
establish the restricted formula theorem, without claiming refinement of the
Rust extraction, ranks, memory accounting or source compiler.

Ordinary formula solving can instead batch independent general reduct queries
through an owned `CompletionExecutor`. `--completion-workers` defaults to one;
values above one opt into CPU batches. The same executor completes GPU residuals.
`--max-completion-scratch-bytes` bounds logical query workspace, reducing active
concurrency when necessary. It excludes immutable theory storage, the outer
candidate cursor, allocator overhead, worker stacks and GPU buffers. The scalar
CPU cursor does not enter this batched workspace contract.

All started slots join before a deterministic commit or failure. Shared search
limits remain cumulative across workers; an incomplete batch preserves its
pending candidates for exact retry. Failure never becomes UNSAT or exhaustion.
Statistics distinguish entered, completed, failed and committed candidates,
residual work, requested/effective workers and the admitted logical workspace.
Coordinator wall time and summed worker time remain different quantities.

## Candidate generation and laziness

An admitted program keeps its finite value domain and predicate signatures
symbolic. The conservative gate carrier S consists of every domain tuple of a
predicate consulted by a true or false gate, including constraint-only gates
and normalized choice heads. Seed membership is exact; an absent tuple is false.
Constructing a seed never enumerates the carrier.

The baseline generator enumerates finite true sets in binary order:
`{}`, `{a}`, `{b}`, `{a,b}`, `{c}`, … . A carry requests one additional gate tuple.
Thus it does not pre-expand S even to produce the first several candidates. The
first empty seed is sent to the oracle alone; later candidates can be batched.
For finite S, uninterrupted exhaustion visits every seed exactly once. This
provides a simple complete fallback, but it is exponential and does not yet
implement must/may cube pruning, learned proposals or a proof-carrying ledger.

The CPU oracle starts with no consequences. It visits all source templates in
each round, joins ordinary positive patterns against the current consequence
relations, and applies exact filters and frozen candidate gates. Bound gates
can reject a partial join early; all gates are checked on completed bindings.
Only resulting heads are materialized. A coalesced delta is published between
rounds. A complete round adding no atom establishes closure and final source
coverage. Constraints and the equality `closure ∩ S = seed` determine acceptance.

The candidate never supplies positive support. A positive cycle cannot justify
itself merely because its atoms appear in a guess. Conversely, the source oracle
can derive a head never suggested by the generator and can reject a constraint
that the generator did not inspect. This separation is the central experiment.

The current CPU joins use predicate-indexed relations and repeated source
rounds. Bound-column indexing in the relational lazy engine, semi-naive delta
scheduling, shared joins across candidate batches and selectivity planning remain
optimization work. Formula eager support already has its own column indexes;
the three grounding paths must be measured separately. The implementation avoids
an upfront ground rule store, but does not imply that every lazy workload is fast.

## GPU profile

The main CLI retains the static least-closure profile below. A separate
[`GpuFormulaOracle`](design/gpu-formula-propagation.md) implements resident
propagation over original Ferraris DAGs, including disjunction and implication.
It freezes candidate truth, narrows Boolean domains and returns original-model
rejection, proper-subset refutation or an explicit residual. It supplies no
complete candidate/countermodel search, objective scoring or source grounding.
The `zetesis-bench formula` experiment includes native CPU completion of every
residual. The opt-in [gate-projection comparison](verification/gate-projection-20260907/README.md)
uses one shared shader scaffold and matched synthetic candidate batches;
`GateProjection::Enumerated` remains the default. Its per-candidate CPU quotas
and serial residual completion differ from ordinary cumulative-budget execution.
Actual-stream characterization of the latter remains separate work.
Explicit GPU selection in ordinary CLI formula solving now composes
bounded original-model proposal batches, GPU propagation and native residual
checking. Pending candidates and verified models awaiting scoring/output remain
accounted for across limits and failures. Portable tests and shader
validation pass. A subsequent [M4 Pro run](verification/metal-formula/20260906T140835Z/README.md)
physically qualified 180 hybrid membership batches and 19,260 candidate
occurrences; 1,434 residuals required CPU checking. That dated primitive benchmark
does not qualify the later ordinary-solver integration or new Rayon baseline.

`GroundProgram::compile` is a separate, explicitly eager operation. It counts the
full atom carrier and all substitutions before expansion, checks supplied
ceilings and preserves all gate tuples. It applies exact filters and deduplicates
ground antecedents. The CLI identifies this profile as static and prints its
atom and rule counts.

Each GPU workgroup owns one frozen world, with 64 lanes and an atomic u32 closure
bitset. Lanes scan strided rules, test frozen gates and currently derived positive
antecedents, and atomically insert heads. Uniform barriers separate full scans;
a scan with no insertions establishes closure. Constraints and the seed equality
are checked only then. There are no floating-point truth decisions, speculative
acceptance or shared candidate truth bits. Monotone in-round insertion may
accelerate a chain without changing its least closure.

The profile is limited to 4,096 atoms and explicitly budgeted batch transport.
It does not implement the relational GPU transformer pipeline proposed in the
design. Host polling has a bounded timeout, but a timeout does not prove that a
device dispatch was physically cancelled. Shader validation is portable evidence;
physical Metal execution is separately recorded for the user-run M4 Pro
[qualification](verification/metal/20260905T214253Z/README.md). Its tested batches
match CPU results; the first sample establishes no advantage over the best CPU
baseline and does not cover the full language or all hardware boundary cases.

Immutable graph buffers are cached by admitted program identity. Equal-shaped
candidate batches reuse transport buffers and their bind group; a different
shape replaces transport while retaining the graph. Per-batch diagnostics record
uploads, allocations and accounted bytes. Driver staging and retired in-flight
allocations are outside the host accounting contract and are documented in the
GPU crate. This is a bounded single-program cache, not an unbounded residency map.

The CLI exposes independent `--grounder auto|lazy|eager` and `--backend` policies.
Explicit eager CPU retains its compiled graph even across automatic GPU fallback;
lazy mode never materializes a rule store. A forced GPU/lazy pair is refused.
The default CLI uses lazy CPU for the first candidate and small batches. Later
batches with at least 32 candidates may initialize a physical GPU and admit the
static profile. Auto fallback reports a reason; explicit backend/vendor requests
never fall back. The threshold is an initial heuristic with no claimed speedup.
`zetesis devices` separates advertised capabilities from actual initialization
and shader execution. Metal, Vulkan, DirectX 12 and GL selection is restricted
to APIs compiled for the host; NVIDIA filtering does not imply CUDA support.

## General formula and source extensions

The separate Ferraris kernel evaluates a finite topological formula DAG at M,
then uses the frozen truth mask to evaluate the reduct at J. It checks that M
models the original roots and that no proper J subset models the frozen reduct.
The result is stable, a failed original root, or a proper-subset counterexample;
resource exhaustion is incomplete. Enumerating countermodels is exponential.
The kernel is independently tested against a materialized tree reduct and has
generated DAG properties with shrinking. The mask-fusion theorem describes its
semantic operation, not the correctness of Rust indices or enumeration code.

The native SAT kernel accelerates search within this reduct architecture. Its
outer query proposes a classical model M. Its inner query searches for a proper
subset J satisfying the frozen Ferraris reduct, using full Tseitin equivalences
and minimizing only semantic atoms. A countermodel rejects M; complete inner
UNSAT establishes minimality. Classical satisfaction alone never establishes an
answer set. Exact outer blocking preserves enumeration, and exhausted resource
budgets remain inconclusive. The CLI automatically selects this oracle for supported formula constructs;
`--oracle countermodel` forces it. Eager lowering remains on the host; explicit
GPU selection batches reduct propagation before native residual search. SAT is a replaceable search implementation, not a change
to the solver's semantic criterion or the experiment's hardware objective.

Query-local Boolean compaction folds constants and aliases only after the original
candidate truth mask is frozen. Generic occurrence counts prioritize branching;
both truth branches remain available and all semantic variables remain in exact
model blocking. The [search design](design/native-search.md) and Lean contracts
distinguish these transformations from rewriting the source theory.

The normal-rule bridge retains ordinary positive bodies and translates frozen
true/false gates into double/single negation. Its guarded variant adds
double-negated supportedness conditions to prune unsupported classical atoms
without constraining reduct countermodels. Generated small programs compare
both translations against the independent lazy closure oracle.

Formula grounding evaluates numeric unary, binary and absolute operations
directly over the existing flat expression plan. This removes temporary boxed
term trees while preserving checked i32 results, operand/error order, locations
and authored work/payload charges. Constructor evaluation, scratch vectors and
join scheduling are unchanged. The [scalar implementation record](verification/scalar-evaluation-20260907/README.md)
separates pinned-evaluator tests and abstract Lean laws from the
[controlled admission ablation](verification/scalar-evaluation-20260907/ablation.md).
That ablation records SEND median admission of 50.346 ms before and 19.367 ms
after, with equal ordered execution subjects and complete models. It excludes
parsing/solving and later integrated language changes; the plain-chain control's
2.87% median increase remains visible. It establishes no ordinary-release,
relational-lazy or GPU performance result.

The finite source extension evaluates supported ground scalar arithmetic and
unambiguous acyclic constants, then expands bounded ordinary fact pools and
intervals into the same S0 templates. It retains original locations and bounds
work, values, templates and provenance independently. It refuses undefined or
overflowing arithmetic rather than reproducing clingo's broader treatment.
Variable arithmetic, richer choices, aggregates and optimization remain outside
this extension. Bundle admission retains original source identities and include edges through
solving, with global scalar constants and cumulative budgets. File
input uses this path; stdin retains single-source admission. Canonical aliases
and redirections that erase a clingo-relevant include distinction are refused.
Extended metadata retains `#defined` and signature/empty `#show` origins;
display selection never merges underlying stable-model identities or counts.

Formula admission additionally compiles bounded ground/conditional `#show`
templates into metadata, independent of the logical IR and possible-positive
carrier. Their public observation engine queries a supplied full `Model`, using
directive-local positive joins, negative/comparison filters and shared themelios
symbols. Output constructors and tuples create no logical atom or support.
Distinct enabled terms coalesce within the term channel; the combined display
preserves equal symbols from the selected atom channel. Term-only shows retain
the default atom selection. Full models and optimal ties remain distinct.

Observation admission, binding, construction, work and output limits are
independent. The CLI buffers an entire observed Answer and its optional cost
vector before emission, so an internal limit/control refusal exposes no partial
Answer or false completion. Ordinary I/O failures can still interrupt writing.
This source route uses eager formula admission with CPU or explicit hybrid GPU
checking; explicit lazy or closure requests remain refused. The observation engine itself is
independent of the oracle that verified the supplied model.

`SourceBundle::load_many` preserves ordered root occurrences within one source
catalog and budget. Includes resolve against the captured working directory first,
then the including directory, with lexical resolution evidence retained alongside
canonical identity. The CLI accepts multiple file operands and has independent
root/file/total-byte/depth limits. Mixed file/stdin input and aliases whose distinct
clingo behavior cannot be represented remain explicit refusals. The
[multi-input record](verification/multiple-inputs-20260905/README.md) states the
checked lookup and ordering behavior.

The separate `admit_formula` / `admit_bundle_formula` APIs add integer-bounded
conditional choice groups, count/sum/sum+/complete-value min/max comparisons and scoped
equality assignments. Numeric choice-head intervals introduce independent local
value slots within the original group; empty expansions retain its bounds.
Evaluated scalar arguments and top-level interval endpoints may depend on
independently bound outer or local variables. Rule/head pools are admitted within
the documented finite expansion profile; unsupported nested generative forms and
closed nonnumeric interval endpoints remain refused. The
[evaluated-head evidence](verification/evaluated-heads-20260906/README.md) records
this extension; the earlier [interval evidence](verification/choice-intervals-20260906/README.md)
preserves its original comparisons and external endpoint timeout.
Dependency-ordered scalar and interval binders generate finite values. Separate
closed integer bounds, whole-opposite-bound flat tuple equalities and one
double-negated interval equality retain every original guard. The
[finite-binding record](verification/finite-bindings-20260906/README.md) distinguishes
the current source profile from its mathematical coverage laws. Evaluated heads,
unsigned default-negated anonymous projections and comparisons preserve their
checked scopes. Each group combines duplicate-head eligibility by disjunction;
its bounds remain constraints and its conditions remain formulas, including
recursive conditions. Compilation computes a bounded possible-positive relation and joins against
it, retaining original atoms, root origins and metadata. This is explicitly eager
formula admission. The CLI selects the appropriate oracle automatically.
The unchanged kr-domains queens variant 01 passes all 92 models on this path.
[Universal body conditionals](design/universal-conditionals.md) now compile as finite
conjunctions of original condition-to-consequent implications. Completed
possible-positive support precedes all local joins. Each condition assignment
retains a disjunction of signed consequent alternatives; finite positive
consequent-only variables/anonymous slots can select local support witnesses.
These cannot bind the outer rule, repair condition safety or provide support.
Empty alternative families are false; completed empty condition domains are true.
Resource exhaustion remains a refusal. Objective-reachable conditional producers
and broader structured witness patterns remain outside this bounded extension.
Pooled analysis input carries `AnalysisBasis::DependencyProjection`; its safety
and class verdicts are not original-source certificates. Candidate-specific
omission is formalized, but the compiler still builds these families eagerly.
Unconditional disjunctive heads of either classical sign admit positive,
default-negated and double-default-negated atom occurrences with closed values,
whole variables, checked scalar arithmetic and top-level numeric/dependent intervals. Each
complete outer binding compiles one implication to its original disjunction;
intervals expand a Cartesian family of whole rules. They are not shifted into
normal rules or flattened into one larger head. All admitted positive head
producers contribute to possible support, without treating possible atoms as
facts. Negative occurrences preserve their original implication-to-falsum nodes
and supply no positive support, including double negation. The ground
`NegativeHeads.lean` grammar proves the necessary positive-support law and
stable-model preservation of its double-negated support guards; this is not a
verification of the Rust compiler or possible-support grounding.
Ordinary singleton heads `not p` and `not not p` use this same signed formula
path, including either predicate sign and evaluated arguments. Their original
implication and frozen polarity remain intact; neither occurrence produces
positive support. The [singleton-head record](verification/singleton-heads-20260907/README.md)
states its complete-model and arbitrary M/J checks.
Explicitly true head conditions can normalize without changing the rule family;
broader conditioned heads, negative choice heads and objective-reachable
disjunctive producers remain typed refusals. Finite rule/head pools retain their
whole-rule or choice-group product semantics. Bound constructor arguments use
the shared finite-value evaluator; nested pool/interval forms keep their own
admission limits. A dedicated
head-element ceiling bounds this source profile.
An assignment introduces one fresh named target, absent from its own tuple and
conditions; other outer variables require ordinary positive bindings. Finite
count ranges and signed subset sums generate possible values, and each retained
value keeps its exact equality formula. Correlated, unrealizable values can enter
this upper approximation but cannot bypass reduct checking. Incomplete support
construction or a resource limit causes refusal.

Exact structural interning shares formula nodes without classical rewrites of
the original theory. After possible-positive support completes, bounded caches
reuse coalesced elements and equality roots for the same aggregate and outer
binding. A shared aggregate-family kernel builds nonnegative thresholds once
for related bounds; signed formulas retain cumulative subset/work limits and
transactional rollback. Numeric min/max assignments preserve real empty extrema.
Sum functions ignore nonnumeric/empty tuples; sum+ also ignores negative weights.
Broader assignments and conditions remain outside the profile. Numeric min/max
source endpoints implicated in six recorded clingo discrepancies remain refused.
Independent multiple aggregate assignments stream products through the existing
bounded cursor while retaining every original equality. Dependent generators and
objective-relevant multiple-assignment producers remain outside that extension.
The main CLI supports both S0 GPU closure and explicitly selected hybrid formula
propagation. The separate formula experiment compares scalar CPU, Rayon and
hybrid membership; exact GPU residual search remains future work.

The independent `zetesis-domain` crate analyzes the exact borrowed themelios
`Program` before construction. Its first lattice uses bounded borrowed symbol
sets or Unknown per signed argument position; ordinary positive variable flow
uses a conservative union fixed point. Unsupported producers widen, and global
resource/context failures clear all finite results. It retains direct producer
provenance and never parses, grounds or invokes a solver. This is an optional
analysis foundation, with no runtime pruning or Lean implementation refinement
claim. See its [crate contract](../crates/zetesis-domain/README.md).

The `zetesis-objective` crate evaluates lifted positive joins over each verified
stable model, coalesces (priority, weight, tuple) keys globally and returns checked
signed costs at fixed descending priorities. The frontend discovers reachable
objective slots from the possible-positive relation. Pure total count/sum
assignment producers can feed positive objective observers under explicit
variable-position and dependency checks. Broader negative or aggregate producer
dependencies are refused; negative constraints remain allowed. The pinned
themelios analysis supplies the dependency graph and retains its own safety
verdict. Clingo-specific assignment binding is checked locally; an upstream
finiteness result alone is not a safety certificate. The
[program-analysis contract](design/program-analysis.md) records this boundary.
Objective rules never enter the support theory. The CLI records exact
cumulative work, retains bounded full incumbents before display selection, and
reports incomplete coverage on any search, scoring or retention stop. A known
better score whose model could not be retained cannot turn an older retained
incumbent into an optimum claim. Positive weak constraints normalize to the same
objective templates and global key semantics. Maximization uses checked weight
negation before global key formation in both exact scoring and candidate-bound
construction; original priorities, tuple components and source direction remain
available. Equal keys across all three source forms combine eligibility.
The [mixed-direction campaign](verification/objective-directions-20260905/README.md)
checks complete costs, unchanged reduct formulas and explicit numeric boundaries.
The preceding fully qualified [93d2575 checkpoint](verification/grounder-tranche-20260907/README.md)
passed all 94 unchanged entry graphs under default limits, preserving their
original display/count/cost and optimum contracts. Hidden full models are not
reconstructed from `#show`. The [new integration record](verification/execution-tranche-20260907/README.md)
tracks qualification of the newer source separately. General factored body construction, retained candidate traversal, root
failed-literal propagation, indexed exact semantic blocks and optional objective
bounds reduce repeated work without changing the original reduct. The
[objective-pruning protocol](design/objective-pruning.md) keeps dominance and
optimal-tie coverage separate from stable-model membership.

## Resources and outcomes

Admission bounds source bytes, syntax traversal, rule-body shape, arity, variables
and domain values. CPU checking bounds logical work and distinct consequences.
Candidate enumeration bounds yielded seeds and discovered gate atoms. Rayon owns
an explicit worker pool and accepts bounded synchronous batches. Static lowering
and GPU transport have their own limits. These are operation/count bounds, not a
claim to recover from every process-wide allocator failure.

Exact model data and coverage are separate. A first model answers existence but
does not establish exhaustive coverage. A limit or cancellation leaves a search
incomplete; it is never converted into rejection or UNSAT. The CLI streams ordinary models. Optimization retains only best tied models
within explicit model/atom/payload limits, scoring only verified stable models
and establishing optimality only after complete coverage of all candidates that
can improve or tie a retained incumbent. Exit 0 means the
requested operation completed, 2 means input/backend/output failure and 3 means
search interruption; these are explicitly not clingo's exit-code conventions.

Human presentation remains a CLI adapter. `-h` and `--help` show everyday options;
`--help-all` shows the existing advanced controls without changing their parser
or defaults. Typed metadata writes style five labels blue and their values italic
gray. Each process stream resolves Auto from its own terminal capability;
injected library writers keep Auto plain, and JSON disables styling. Ordinary
diagnostic writes pass through without buffering or classification. See the
[CLI stream contract](../crates/zetesis-cli/README.md) for writer-prefix failures
and semantic/publication evidence.

## Formal and hardware boundary

The [Lean development](../proofs/README.md) proves the normalized reduct semantics,
composition laws, final-coverage lazy bridge, supplied complete search certificates
and abstract legal event traces. The independent reference campaign and production
conformance tests supply executable evidence separately. No theorem currently
connects the concrete Rust structs, source adapter or shader to the Lean types.

The native neuromorphic specification includes exact support counters, frozen
epochs, reliable or fault-detecting transport and semantic termination checks.
SpiNNaker2 firmware access and runtime integration, and Loihi 2 instruction/state
mapping and current tooling access, remain qualification gates. No native backend
or neuromorphic performance result is included in this prototype.
