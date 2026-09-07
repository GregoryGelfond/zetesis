# zetesis

[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)

ζήτησις, *inquiry/search* — candidate-directed answer-set solving through the reduct.

zetesis explores how answer-set programming can use parallel and event-driven
hardware. A generator proposes a candidate. An exact oracle freezes its truth
choices and checks minimality against the reduct. For the current normal-rule
source profile, this specializes to least closure followed by constraint and
candidate agreement checks. Only then is an answer set returned.

The goal is a complete, massively parallel ASP architecture with GPU-hosted
semantic computation and the operational assurance required by mission-critical
systems. Exact reduct semantics, comprehensible composition and bounded failure
behavior govern that work. The current hybrid implementation is an intermediate
milestone; publication prospects are secondary to the solver itself. Exact algorithms
for recognized program classes are part of the architecture: a checked lowered
certificate may justify closure or support checking, with the general reduct
oracle retained where required. A bounded [certified support checker](crates/zetesis-ferraris/README.md)
now checks eligible completed normal/choice theories through the Rust API. Its
corpus experiment preserves all 652 models, costs and optimum ties on 13 inputs
while eliminating their 652 inner countermodel queries. A
[matched library measurement](docs/verification/tight-wall-performance-20260906/README.md)
finds about 1.66× speedup on the larger task-allocation input and 1.96× on two
queens encodings, with certificate construction included. Single-model and small
cases need selective treatment. Automatic CPU selection now uses the checked
certificate in ordinary solving. The later [ordinary invocation comparison](docs/verification/first-six-checkpoint-20260907/ordinary-comparison.md)
measures 4–42% lower median latency on the six queens encodings against the prior
installed baseline; SEND + MORE = MONEY is essentially unchanged. These CPU
measurements include admission and output; they establish no GPU speedup.

In the normal-rule specialization, the CPU oracle works directly with relational
source templates. It binds rules
against consequences as they become available, without first building a ground
program. The candidate stream also starts sparsely: its first check needs no
enumeration of the possible gate tuples.

## Restore a route

A clinic is cut off because both approach bridges need repair. A crew can repair
one bridge. Which repair plans restore a route from the depot to the clinic?

```clingo
road(depot,north).
road(depot,south).

{ repair_north }.
{ repair_south }.
:- repair_north, repair_south.

open(X,Y) :- road(X,Y).
open(north,clinic) :- repair_north.
open(south,clinic) :- repair_south.

reachable(depot).
reachable(Y) :- reachable(X), open(X,Y).
:- not reachable(clinic).
```

```console
$ zetesis examples/network-repair.lp --models 0
```

There are two answer sets: one contains `repair_north`, the other
`repair_south`. Each also contains the road facts, its open connections and the
four reachable locations. Neither an unsupported reachability cycle nor a plan
requiring both repairs is accepted. The complete model sets for this unchanged
example are compared with clingo in the regression suite.

The [example](examples/network-repair.lp) combines a planning choice, a resource
constraint and recursive consequences. The default returns one plan;
`--models 0` requests every plan. Standard input is accepted as `-`. Limits
produce an explicit incomplete result; UNSAT requires exhaustive search.

## Eight queens from kr-domains

The unchanged [eight-queens encoding](validation/corpus/kr-domains/standalone/n-queens/variant-01.lp)
is one of the complete kr-domains cases supported by zetesis. It chooses exactly
one queen in each row and column, then rules out shared diagonals:

```clingo
row(1..8).
column(1..8).
1 <= { queen_at(R,C) : column(C) } <= 1 :- row(R).
1 <= { queen_at(R,C) : row(R) } <= 1 :- column(C).
:- queen_at(R1,C1), queen_at(R2,C2), R1 < R2, R1 - C1 = R2 - C2.
:- queen_at(R1,C1), queen_at(R2,C2), R1 < R2, R1 + C1 = R2 + C2.
#show queen_at/2.
```

```sh
zetesis --models 0 validation/corpus/kr-domains/standalone/n-queens/variant-01.lp
```

All **92 answer sets** match clingo, including the complete model count and
displayed queen positions. The oracle rejects candidates whenever a proper subset satisfies their
Ferraris reduct. Reduct minimality remains the acceptance criterion. This richer
source path uses eager grounding and defaults to CPU execution. An explicit
`--backend metal` uses general GPU propagation with exact CPU completion of
residual queries. The route-repair example above also runs on the Metal closure
specialization.

## Assign tasks at minimum cost

The unchanged [task-allocation scenario](validation/corpus/kr-domains/scenarios/task-allocation/variant-01/01-basic.lp)
chooses one compatible agent for each task and minimizes assignment cost. Its
original encoding contains:

```clingo
1{ assigned_to(A,T) : agent(A), compatible_with(A,T) }1 :- task(T).
#minimize{ C,A,T : assigned_to(A,T), cost_of(A,T,C) }.
```

```console
$ zetesis validation/corpus/kr-domains/scenarios/task-allocation/variant-01/01-basic.lp
Answer: 1
assigned_cost(a1,t1,3) assigned_cost(a2,t2,2) assigned_to(a1,t1) assigned_to(a2,t2)
Optimization: 5
```

The final status is `OPTIMUM FOUND` with exhausted coverage. The solver checks
every candidate that can improve or tie a verified incumbent before proving the
optimum. Exact objective bounds prune worse candidates. Objectives read verified
models and cannot provide atom support. `--models 0` displays every tied optimum.
The full corpus comparison checks costs, optimal answer sets, counts and original
contracts, including cases where an agent takes several tasks.

## Install and run

Build and install native release binaries once with Rust 1.97.1:

```sh
./scripts/install.sh
zetesis --help
zetesis --version
zetesis devices
zetesis examples/network-repair.lp --models 0
zetesis examples/network-repair.lp --models 0 --stats
zetesis examples/network-repair.lp --models 0 --json
zetesis examples/network-repair.lp --backend metal --grounder eager --models 0
```

The installer uses `~/.local/bin` by default; an alternative binary directory is
its optional argument. The installed commands run directly without Cargo or a
Rust toolchain at runtime. GPU support is included in the normal installation.
Device selection and pipeline initialization happen inside the normal solve,
immediately for an explicit GPU request or on first eligible use in automatic
mode. No setup or qualification script is required. `zetesis devices` is an
optional inventory command, not an initialization step.

`--stats` writes settings, available search counters, completion status and
host phase timings to stderr while preserving answer-set output on stdout.
Unavailable counters and untracked automatic CPU/GPU transitions are labeled
explicitly. A readable timing summary separates source preparation, actual eager grounding,
solving (including setup and waits), and output. Lazy joins are explicitly
interleaved with solving; an unavailable grounding duration is not reported as
zero. The timing excludes source loading and is not a GPU kernel measurement. The phase section separates admission,
candidate generation, original validation, GPU host calls, exact reduct checking,
objectives and output. Unentered phases remain unmeasured; failed attempts retain
their timing. Without `--stats`, no timing clock is read. The validator captures
these optional measurements separately from its answer-parity decisions.

`--json` selects a versioned streaming document for machine consumers. It retains
full answer sets separately from shown atoms and terms, objective costs, completion
and partial failure evidence; `--stats` also adds typed timing and execution data.
Human output remains the default. [The JSON contract](docs/verification/json-output-20260907/README.md)
states numeric precision, per-record limits and transport-failure behavior.

For programs using reduct closure, automatic hardware selection checks the first candidate on the lazy CPU path. Later
batches of at least 32 candidates can use a detected physical GPU, with an
explained CPU fallback if the device or static profile cannot be admitted.
This provisional threshold is not a measured performance crossover. Explicit
`--backend cpu`, `metal`, `vulkan`, `dx12`, `gl`, `nvidia`, or `gpu` overrides are
available; explicit GPU requests fail if unavailable. NVIDIA selection uses a
supported wgpu graphics API, not CUDA. The [CLI guide](crates/zetesis-cli/README.md)
details selection, diagnostics, resource limits and exit codes.

`--grounder auto|lazy|eager` selects materialization independently. Lazy CPU
evaluates source joins; eager CPU uses the packed static oracle. Explicit
GPU/lazy combinations are currently refused. The default permits lazy CPU
and the current static GPU profile.

For formula solving, `--completion-workers 4` enables bounded Rayon completion
of independent reduct queries; `--batch-size` controls candidate batches.
`--max-completion-scratch-bytes` bounds their admitted logical workspace and can
reduce active concurrency. It is separate from process memory and GPU storage.
The scalar CPU default uses one worker; explicit GPU solving also uses these
settings for residual completion. `--stats` reports requested and effective
concurrency, workspace and complete candidate accounting.

## Compatibility target

The current milestone is **clingo language and solving compatibility, excluding
theory terms and Python/Lua scripting**. Rust `@` functions are a planned first-class
extension through themelios's shared API. The original non-clingcon encodings and scenarios in
kr-domains are a required first corpus, including optimization and observable
model contracts; passing that corpus alone will not establish full compatibility.
The [corpus compatibility contract](docs/verification/kr-domains-compatibility.md)
pins that target. The [Ferraris/clingo extension](docs/design/ferraris.md) supplies
the forward semantic basis for choices and aggregates, with general reduct
minimality checking where the Horn least-closure specialization is insufficient.
The implemented source profiles cover the complete non-clingcon kr-domains target;
broader clingo language compatibility remains in development. A reviewed
[theory-extension design](docs/design/theory-propagators/README.md) keeps future
clingcon/clingo-dl support open through both direct source admission and the
themelios Rust API; it adds no runtime theory capability yet.

## Status

zetesis is a working experimental solver with native CPU and Metal execution.
**Full clingo compatibility is the active implementation milestone.** The latest
complete corpus campaign passes **all 94 unchanged non-clingcon kr-domains cases**,
including their original model, count and optimum contracts, against external
clingo. All cases complete within the default limits. The complete references
also run as portable CLI regressions. This corpus milestone does not establish
full clingo language compatibility or a GPU performance advantage.

| Capability | Current boundary |
|---|---|
| Reduct closure specialization (**S0**) | Finite, safe normal rules, constraints, singleton choices, `not`/`not not`, closed values, equality and inequality; variables need ordinary positive body bindings. |
| Strong negation | Distinct signed predicates across admitted source profiles, with coherence integrity constraints; signed `#defined`/`#show` and signed observation constructors. Default negation remains independent. |
| Automatic scalar source admission | Bounded scalar constants/arithmetic, interval and pooled facts, ordered input files and original include bundles with global constants, `#defined` and signature/empty `#show`. Output selection preserves distinct underlying models. |
| Automatic formula source admission | Finite conditional choices with integer bounds and dependent numeric choice-head intervals, evaluated signed disjunctive heads with `not`/`not not` occurrences and bounded top-level intervals, body count/sum/sum+ and complete-value min/max comparisons with scoped equality assignments, scalar/interval binders, evaluated normal heads, anonymous negative projections over unsigned predicates, scalar ordering and flat tuple equality. Boolean guards and signed comparison chains work in ordinary bodies and admitted choice/aggregate conditions; finite scalar/flat-tuple equalities, double-negated interval equalities, closed numeric chains and separate closed bounds can generate bindings while retaining the complete guard; finite universal body conditionals retain each original implication after complete local bindings. Finite rule/head pools preserve whole-rule products and choice-group bounds. Explicitly true disjunct conditions and finite function `#count` heads with static eligibility and checked tuple/atom correspondence are supported. Direct Ferraris formulas retain original source evidence. |
| Closed structural values | Bounded signed functions and tuples in logical atoms, whole-variable copies, comparisons, aggregate/objective tuple keys and observations; constructor sign is independent of predicate sign. Constructor pattern extraction and variable-containing construction remain refused. |
| Observation queries | Ground and positively bound term/conditional `#show`, including output constructors and tuples. Separate atom/term channels preserve duplicate symbols and distinct full models; eager formula execution uses CPU or an explicitly selected hybrid GPU oracle. |
| Optimization | Admitted `#minimize`, `#maximize` and weak constraints on verified stable models; normalized global tuple keys, priorities, bounded incumbent retention and exact candidate cost bounds retaining all optimal ties. Signed candidate bounds can use nonnegative threshold normalization without changing the original reduct. Total aggregate observers use explicit dependency checks. |
| CPU execution | Lazy relational joins or explicit eager lowering; bounded Rayon closure batches and exhaustive candidate coverage. Ordinary formula solving and GPU residual checking can use a reusable shared-budget Rayon completion executor with explicit worker and logical-scratch limits. Scalar formula completion remains the default. |
| Certified class checking | A checked producer/rank certificate permits exact support-based membership on eligible completed normal/choice theories. Ordinary CPU formula solving with `--oracle auto` uses this certificate when available; setup and checking share the cumulative work budget. Cyclic or opaque theories and uncompleted certificate checks retain exact reduct completion; explicit `--oracle countermodel` retains the general route. |
| GPU execution | Static integer oracle physically qualified on Metal, with resident reuse and a 4,096-atom limit. General Ferraris propagation integrates into ordinary explicit GPU solving with exact CPU residuals; the retained 7 September M4 Pro Metal build passed all 94 original corpus cases with both one and four completion workers, plus four focused integration/device tests ([qualified binary and evidence](docs/verification/metal-requalification-20260907/README.md)). Other compiled wgpu APIs have no local hardware qualification. |
| General reduct semantics | Finite Ferraris formula checker and native candidate/countermodel search; normal-rule source translation is connected through explicit eager lowering. |
| Remaining language/search work | Broader disjunctive heads and objective-dependent disjunction, broader assignments/conditions, constructor patterns and variable-containing construction, broader directives, exact undefined-arithmetic behavior, further search and GPU acceleration. |

The checked source boundary also has an [exact upstream regression collection](validation/upstream/clingo-5.8.2/README.md), with original clingo assertions and complete model references. All 24 selected inputs parse and raise with the unchanged themelios pin; 21 now pass complete model parity and three retain explicit native-admission refusals. This selected set is not a percentage of language compatibility. A [finite GPU gate experiment](experiments/gate-transfer/README.md) compares exact transfer alternatives; its proposed shader is validated independently and remains unapplied.

Unsupported constructs and exhausted admission budgets produce located
refusals. Undefined or overflowing arithmetic is currently refused rather than
implementing clingo's full behavior. A search budget stop produces an explicitly
incomplete result. These boundaries apply regardless of hardware selection.

The [first-six ordinary CPU comparison](docs/verification/first-six-checkpoint-20260907/ordinary-comparison.md)
retains 728 complete samples on 13 original inputs. Against the installed baseline,
checked support certificates reduce the six n-queens median times by about 4–42%.
SEND + MORE = MONEY stays near 73 ms versus clingo's 13 ms; admission accounts
for about 76% of its instrumented driver time. Four completion workers do not
provide a consistent benefit on this set. All model/display/count/optimum
contracts agree, and separate RSS samples are retained. These observations
establish neither a general solver advantage nor a GPU speedup.

The subsequent [library views and telemetry comparison](docs/verification/estate-boundaries-20260907/ordinary-comparison.md)
retains two complete 728-sample campaigns against that first-six release. All
answer contracts still agree; the small timing differences establish no new
speedup. The new separate SEND diagnostic places about 49.6 ms in eager grounding
and 16.3 ms in solving. Human output remains the timed default; JSON is optional.

The [previous seven-input comparison](docs/verification/cpu-performance-20260906-parallel-plans/README.md)
retains 525 runs, including traveling-salesman and a larger task-allocation input
outside the new thirteen-input set. The two case sets and tested releases remain
explicit; their results should not be substituted for one another.

A [matched quota specialization](docs/verification/scalar-quota-specialization-20260906/README.md)
restores scalar performance after shared completion introduced a per-operation
mode check: n-queens drops from about 139 ms to 126 ms, with identical answers
and search work. Private static policy selection retains one shared search
implementation. The [first shared-completion characterization](docs/verification/shared-completion-characterization-20260906/report.md)
uses the preceding intermediate build: four workers take about 4.0–4.7 times the
scalar time on tiny eight-atom formulas. All 612 complete results and accounting
checks agree. The executor now has bounded logical scratch and ordinary CLI
controls; that historical measurement is not relabeled for the current build.

The [controlled signed-bound ablation](docs/verification/signed-objective-bounds-20260906/README.md)
compares identical native sources with only candidate-bound normalization disabled
in the control. Four original traveling-salesman cases improve by paired median
factors of **4.28×–18.37×**, while the SEND, task-allocation and queens controls
show no meaningful change. All 336 invocations preserve complete answer/count/cost
contracts. This is a general representation improvement after objective-key
coalescing; the original theory and reduct checks are unchanged. The
[earlier CPU comparison](docs/verification/cpu-performance-20260906/README.md)
and [clause-scan ablation](docs/verification/clause-validation-20260906/README.md)
remain separate historical measurements.

The [retained Metal requalification](docs/verification/metal-requalification-20260907/README.md)
for solver SHA-256 `ae58cd1eaf4324126ae4106b36a9d8b5edb56502728e470d2c19d2681d8e6ddc`
passed 94 cases with one completion worker and 94 with four. Each campaign
executes 105 GPU batches over 2,358 candidates; every residual completes
exactly and no candidates remain pending. All four focused physical tests
also passed. These correctness runs provide no new performance comparison.
The subsequent views/timing build has no new physical Metal qualification.

The recorded [ordinary Metal campaign](docs/verification/metal-batched-formula/20260906-corpus/README.md)
passes all 94 original cases: 79 exercise GPU membership and 15 prove UNSAT in
outer search. It records 105 batches and 2,358 candidates, with 1,318 exact CPU
residual completions. A matched scalar/Rayon/Metal membership run has Rayon
fastest in 20 of 30 synthetic shapes, scalar fastest in 10 and Metal fastest in
none. This supports retaining CPU automatic selection while profiling the
remaining costs; it does not rule out GPU gains on larger or different workloads.

The [updated ordinary CPU phase profile](docs/verification/phase-profile-20260906-completion-bindings-quota/README.md)
retains three further complete 94-case comparisons. Candidate generation accounts
for about 36% of measured driver time, admission/materialization
31%, exact reduct checking 21% and objective feedback
less than 1%. These exploratory host measurements guide the next source and
parallel-execution work; they establish no GPU crossover.

The implementation includes:

- An immutable relational core and sparse, program-bound candidates.
- A themelios source adapter with explicit admission limits and retained origins,
  using pinned themelios program analysis for structural dependency checks.
- A separate `zetesis-domain` crate for bounded argument-domain analysis over
  the exact borrowed themelios `Program`. It reports conservative finite sets or
  unknown information and has no solver dependency; runtime pruning remains future work.
- An exact lazy CPU oracle, a packed static CPU baseline and a bounded Rayon pool.
- A general finite formula-reduct checker with proper-subset rejection witnesses,
  with retained candidate traversal, root propagation, indexed exact model blocking
  and independently checked objective bounds. Oracle
  selection is automatic; advanced `--oracle closure|countermodel` controls are
  available. Explicit lazy/GPU requests are preserved or refused.
- Bounded possible-positive relational grounding for formula construction, and
  lifted objective joins against each verified stable model. Aggregate elements
  coalesce by complete tuple identity; bounded threshold formulas handle
  nonnegative sums, with an exact bounded subset translation for signed sums.
  Related assignment queries share threshold computation and exact structural
  formula nodes; bounded caches are scoped to a completed possible-positive relation.
- Incremental exhaustive candidate enumeration with explicit partial coverage.
- Compositional exact batch completion with a reusable shared-budget Rayon executor,
  preserving proposal order and all-or-nothing batch commitment. Its library API
  includes bounded scratch admission and ordinary CLI controls through
  `--completion-workers` and `--max-completion-scratch-bytes`. See the
  [original completion boundary](docs/verification/completion-executor-20260906/README.md)
  for the historical library-only checkpoint.
- Additive detailed Rust failure APIs retain the original typed cause, optional
  semantic progress, attempted timings and secondary diagnostic failures. Verified
  models, complete publications and established coverage remain distinct.
- Bounded observation queries over verified full models, using themelios symbols
  for output values without adding displayed constructors to the logical carrier.
- A wgpu oracle using integer bitsets and one workgroup per candidate, enabled by
  default in the CLI. This first GPU profile uses **explicit static grounding**
  and supports at most 4,096 atoms; immutable graphs and equal-shaped transport
  buffers remain resident across batches. Lazy GPU joins remain future work.
- A separate resident GPU primitive for all five Ferraris DAG operators, with
  sound Boolean-domain narrowing and explicit residual queries. The
  [formula experiment](docs/design/gpu-formula-propagation.md) measures GPU
  propagation plus exact CPU residual checking. Its physical M4 Pro qualification
  passes. Ordinary explicit GPU formula solving now batches original-model
  proposals before propagation and exact CPU residual checking; pending candidates
  and queued verified models remain accounted for. Its ordinary Metal corpus
  and focused device checks pass. The benchmark also measures a reusable Rayon
  membership pool separately from scalar CPU and hybrid execution. Ordinary
  solving now exposes optional host phase measurements; demonstrating a useful
  full-solve GPU crossover and deployment stress qualification remain targets.
- A Lean specification with **613 checked theorems**, including the reduct
  acceptance criterion, lazy final coverage, finite aggregate-assignment coverage,
  exact structural DAG sharing, signed coherence, first-true witness scanning
  and completion-preserving frozen-query propagation, plus batch coverage and
  occurrence conservation under explicit producer/checker assumptions, and
  contextual replacement of fully evaluated atom-free guards preserving original
  and arbitrary frozen-reduct truth. These guard laws assume total evaluated
  results; source binding, machine arithmetic and compiler refinement remain
  separate obligations. Additional laws cover signed candidate-bound normalization
  after full-key coalescing, universal conditional semantics and safe fixed-candidate
  specialization. Static reuse across candidates requires a uniform coverage
  premise. Fixed-countermodel feedback laws are checked
  separately from the production search implementation.

**Physical Metal execution passed on an Apple M4 Pro:** the example's two models
matched lazy CPU output, and 108 GPU batches (11,556 candidate checks) matched
the packed CPU oracle. Warm batches reused resident graph and transport buffers.
The [raw qualification record](docs/verification/metal/20260905T214253Z/README.md)
includes binary hashes and timings. The best CPU median was faster in all 18
microbenchmark cases; an advantage over the stronger CPU baseline remains to be
demonstrated. No full-domain speedup or energy improvement is claimed.

**The general Ferraris primitive also passed on M4 Pro:** 180 Metal batches
containing 19,260 synthetic candidate occurrences matched native membership;
1,434 residual queries were completed on CPU. All 150 warm batches reused
resident buffers. Two choice batches were faster than the serial CPU baseline
(2.44× and 2.61× paired median ratios); CPU was faster in the other 28 cases.
The [complete formula run](docs/verification/metal-formula/20260906T140835Z/README.md)
retains every result and limitation. This is a synthetic membership result,
not full GPU solving or a corpus speedup.

Native SpiNNaker2 and Loihi 2 mappings are specified as further experiments.
The [feasibility assessment](docs/design/neuromorphic-feasibility-20260906.md)
examines hardware access, simulation fidelity and exact completion protocols;
neither has an implemented device backend. A
[GPU Datalog review](docs/design/gpu-datalog-20260906.md) identifies persistent
relations, delta scheduling and bounded joins as shared CPU/GPU opportunities.
The future Rust API is intended to integrate beneath themelios-solve's shared
contract. [Integration](docs/design/themelios-solve-integration.md),
[Rust function execution](docs/design/rust-functions.md)
and [incremental sessions](docs/design/incremental-sessions.md) are design work;
embedded Python/Lua scripting remains excluded.
The typed themelios `Program` is also the input to the separate
[domain-analysis work](docs/design/domain-analysis.md), before grounding or solving.
[Demand planning](docs/design/demand-and-magic-sets.md) uses `#show` as the modeler's
implicit observation query while retaining whole-program completion obligations.
[Source transformations](docs/design/source-transformations.md) currently use
themelios's rewriter for constant and closed-arithmetic normalization. Body
factorization operates during finite lowering. A general ngo-style
`Program`-to-`Program` optimization pipeline remains planned; each additional
pass needs its own semantic preservation contract.
The Lean proofs establish semantic
contracts; they are not a proof that the Rust, themelios adapter, or WGSL refines
those contracts. See the [implementation status](docs/implementation.md) and
[verification record](docs/verification/status.md) for the precise boundary. The [production assurance audit contract](docs/verification/audit-contract.md)
sets the bar for semantic assurance, clear code for an ASP/reduct reader, explicit
function composition, resource behavior, recovery and operational reliability.

themelios provides the reference quality floor for the entire stack. The
[standards alignment](docs/design/estate-alignment-20260907.md) distinguishes
the checks enforced here, demonstrated improvements and remaining assurance work. The
[v1.0 boundary draft](docs/verification/v1-release-boundary-draft-20260906.md)
sets out the language, execution, interface and qualification decisions to make
at parity. Mathematical performance, pure Rust `@` functions and competition
readiness are explicit objectives alongside parallel and GPU execution.

The [library boundaries checkpoint](docs/verification/estate-boundaries-20260907/README.md)
adds bounded typed model/JSON views, separate eager grounding/solving statistics,
a structured Lean proof pilot and a pinned proof CI job. It passes the 94 original
corpus contracts and 48 external oracle tests. Its hosted coverage is **91.68%
workspace** and **92.10% CPU-only CLI**, retaining both independent **91% floors**.
Broader writer-free solve orchestration and ASPIF remain planned.

The [coverage workflow](docs/verification/coverage.md) complements the semantic
tests with separate full-workspace and CPU-only measurements, an enforced floor,
and inspectable uncovered-code reports. The
[first six language-target checkpoint](docs/verification/first-six-checkpoint-20260907/README.md)
passes **91.70% workspace line coverage** and **93.39% CPU-only CLI line coverage**,
each independently required to pass a **91% floor**, with no additional exclusions.
Qualification records 994 passing Rust test executions across both feature
configurations, 48 external clingo comparison tests and 51 Python regression
methods. Per-crate figures remain explicit; line coverage
does not measure shader or physical-device execution. The separately retained
7 September build passed [physical Metal requalification](docs/verification/metal-requalification-20260907/README.md):
94 corpus cases per completion-worker setting (one and four), plus four focused
device/integration tests. The [overall checkpoint assessment](docs/verification/stocktake-20260907-first-six.md)
compares every objective with implemented, tested and planned boundaries.

Formatting, pedantic Clippy and strict
rustdoc follow the estate's authored-code baseline. Targeted
[mutation audits](docs/verification/objective-mutations-20260905/README.md)
check whether critical assertions detect deliberately introduced defects.

## Build and check

Rust 1.97 or newer is required; this checkout pins 1.97.1 for reproducibility.
The lockfile records the tested dependency
versions. No sibling estate checkout is needed to build zetesis.

```sh
cargo build --locked -p zetesis-cli
cargo test --locked --workspace --all-features
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
```

To select the static GPU profile on a machine with an accessible physical GPU:

```sh
zetesis examples/network-repair.lp --backend metal --grounder eager --models 0
cargo test --locked -p zetesis-wgpu --test hardware -- --ignored --nocapture
```

The [proofs](proofs/README.md) pin Lean 4.33.1 and require no external Lean
packages. With that toolchain installed:

```sh
./scripts/check.sh proofs
```

The independent historical [reference interpreter](validation/reference/README.md)
lives outside the production Cargo workspace. It preserves the finite semantic
campaign that informed the design.

With clingo installed, run the independent end-to-end model comparison:

```sh
cargo test --locked -p zetesis-cli --test clingo -- --ignored --nocapture
./scripts/check.sh oracle
./scripts/validate.sh --report target/full-compatibility.json
```

The CLI campaign compares **393 complete source cases through both enumeration
and countermodel checking**: 262 common-profile programs, three attributed rule excerpts,
the network-repair example and 127 extended scalar/display cases. Additional
campaigns compare 28 original include graphs, 35 metadata cases and 75
conditional-choice cases. An independent exhaustive-reduct campaign covers
403 valid small programs and 26 inputs refused by both systems. Rule excerpts
remain regressions, not evidence of full-domain support. clingo is used only by
these optional tests and the independent corpus validator.

All 108 required original corpus sources are
[vendored](validation/corpus/README.md) with pinned hashes and their license.
The native loader traverses all 94 entry graphs in the portable regression suite.
The full external reference campaign passes all 94 original case contracts.
Native zetesis passes all **94 unchanged cases** with complete displayed-model
multisets, counts, costs and original contracts, under the default limits.
Full-target validation exits successfully only when every required case passes.
`--reference-only` cannot establish native compatibility.
See the [test strategy](docs/verification/testing.md),
[compatibility matrix](docs/verification/clingo-compatibility.md) and
[corpus assessment](docs/verification/kr-domains.md).

## Design

The [design specification](docs/design/zetesis.md) develops the transformer
algebra, lazy materialization contract, candidate search, GPU placement and
neuromorphic execution profiles. All Lean sources and their axiom audit are
maintained in this repository alongside it.

The useful analogy with modern ML is composable operations with predictable
memory access and parallel execution. Exact symbolic truth remains the oracle's
contract. Whether these operations produce a hardware advantage is a measured
question for the experiment.

## Built on

[themelios](https://github.com/GregoryGelfond/themelios) supplies source identity,
parsing and structured program syntax. zetesis uses morphe's `87c11a3` Git pin.
[Rayon](https://github.com/rayon-rs/rayon) runs independent CPU candidates;
[wgpu](https://github.com/gfx-rs/wgpu) supplies the GPU boundary.

zetesis is a separate experiment from apokrisis and a member of the same estate
as themelios, keryx and morphe. Its [library-first design](docs/design/library-first-20260907.md)
uses typed semantic models, composable operations, explicit limits and refusals,
and human or machine views over those models. The [contribution contract](CONTRIBUTING.md)
sets the same clarity, lint, documentation and verification bar. The semantic
kernels are reusable today; extracting ordinary orchestration from CLI-shaped
configuration remains an explicit architectural task. Standalone ground/solve
and ASPIF interchange are specified follow-on capabilities.

## License

[MIT](LICENSE). Copyright © 2026 Gregory Gelfond.
