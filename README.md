# zetesis

[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)

ζήτησις, *inquiry/search* — candidate-directed answer-set solving through the reduct.

zetesis is an independent Rust answer-set solver exploring lazy materialization,
parallel CPU execution and GPU computation. A generator proposes an
interpretation; an exact oracle checks the original program and its frozen
reduct. Only an interpretation that meets the stable-model criterion is returned
as an answer set. Clingo is an external test oracle, never the runtime grounder
or solver.

**The reduct is the architectural foundation.** For safe normal rules, checking
specializes to least closure, constraints and candidate agreement. Richer
programs use finite Ferraris formulas and proper-subset countermodel checking.
A checked certificate can justify a cheaper exact test for an eligible program
class. These are semantic specializations of the same acceptance criterion.

The aim is a massively parallel ASP system with the assurance required by
mission-critical applications. The current implementation is experimental and
hybrid: general formula grounding and candidate search run on the host, with
optional GPU propagation and exact CPU completion. Full language parity, general
lazy formula construction, a demonstrated full-solve GPU advantage and deployment
qualification remain open.

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

```sh
zetesis examples/network-repair.lp --models 0
```

The [complete example](examples/network-repair.lp) has two answer sets: one
repairs the northern bridge, the other the southern bridge. Each includes its
facts and recursive consequences. The solver rejects unsupported reachability
and a plan requiring both repairs. The regression suite compares complete models
with clingo, rather than just their displayed repair choices.

A [kr-domains task-allocation scenario](examples/kr-domains/scenarios/task-allocation/variant-01/01-basic.lp)
adds an objective:

```clingo
1{ assigned_to(A,T) : agent(A), compatible_with(A,T) }1 :- task(T).
#minimize{ C,A,T : assigned_to(A,T), cost_of(A,T,C) }.
```

```sh
zetesis examples/kr-domains/scenarios/task-allocation/variant-01/01-basic.lp
```

Its optimum assigns `a1` to `t1` and `a2` to `t2`, with cost **5**. Objectives
score verified stable models; they supply no atom support. Exact bounds may
prune candidates that cannot improve or tie an incumbent. `OPTIMUM FOUND`
requires complete search, and `--models 0` displays every tied optimum.
The [eight-queens example](examples/kr-domains/standalone/n-queens/variant-01.lp)
likewise preserves all **92 answer sets**.

The self-contained [kr-domains collection](examples/kr-domains/README.md) contains
all 94 non-clingcon cases and their 14 shared encodings. Only elenctic annotation
comments have been removed; a pinned manifest retains their test contracts,
original and cleaned source hashes, and exact deletion provenance. Rust validation
uses these examples by default. The independent original/clean clingo comparison
passes every case with the same costs and selected display multiplicities.

## Install and run

From a checkout, install the pinned Rust 1.97.1 toolchain and native release
commands once:

```sh
rustup toolchain install 1.97.1 --profile minimal --component rustfmt,clippy
./scripts/install.sh
zetesis --help
zetesis --version
zetesis examples/network-repair.lp --models 0 --stats
zetesis examples/network-repair.lp --models 0 --json
```

The private repository and pinned themelios dependency require GitHub read
access for the initial checkout/build. No sibling estate checkout is needed.
The installer defaults to `~/.local/bin`; add that directory to `PATH` if the
installer reports it missing. Pass a different binary directory as its argument.
Installed commands need neither Cargo nor a Rust toolchain at runtime. GPU
support is included; normal solving initializes its own device and pipelines.

Give multiple files in order, or use `-` for standard input. Without objectives,
the default requests one answer set; `--models 0` requests exhaustive enumeration.
Human output is the default. `--json` exposes a versioned streaming view retaining
full models separately from displayed symbols, objective costs, completion and
partial failure evidence. A resource stop is **incomplete**, never an UNSAT proof.
The [CLI guide](crates/zetesis-cli/README.md) describes stream contracts and exit
codes.

`-h` and `--help` show everyday solving options. Use `--help-all` for the full
oracle, worker, batch and resource controls; these options remain available in
ordinary invocations.

On a capable terminal, human answer headings use cyan with a bold `Answer:` label;
optimization metadata uses italic green. Solve metadata on stderr uses blue
labels and italic gray values. The untagged `SATISFIABLE` and `UNSATISFIABLE`
verdicts use bold italic gray. Colors come from your terminal palette.
`--color auto|always|never` controls styling. Automatic mode resolves stdout and
stderr independently, respects a nonempty `NO_COLOR` and `TERM=dumb`, and leaves
each redirected stream plain. JSON never contains styling.

`--stats` adds host timings and available work counters to stderr. It separates
source preparation, eager grounding, solving and output. Formula grounding has
additional attribution for support completion, rule instantiation and other
materialization phases. Lazy joins are explicitly interleaved with solving;
unentered or unmeasured work is not reported as zero. Timings exclude source
loading, include instrumentation overhead, and are not GPU kernel measurements.
Statistics are also available as typed Rust values and in the JSON view.

## Select grounding and execution

```sh
zetesis examples/network-repair.lp --backend cpu --grounder lazy --models 0
zetesis examples/network-repair.lp --backend metal --grounder eager --models 0
zetesis devices
```

`zetesis devices` is an optional inventory command, not a setup step.

| Control | Current behavior |
|---|---|
| `--grounder auto\|lazy\|eager` | Relational execution can join source templates lazily or lower eagerly. Explicit lazy GPU execution composes host joins with per-world device consequences. Formula execution currently requires eager materialization. |
| `--backend auto` | The closure route starts on lazy CPU; later eligible batches may use an accessible GPU. Formula solving selects CPU automatically. The closure batch threshold is provisional, not a measured crossover. |
| Explicit backend | `cpu`, `metal`, `vulkan`, `dx12`, `gl`, `nvidia` or `gpu`. An unavailable required device is an error. NVIDIA uses a supported wgpu API, not CUDA. |
| `--completion-workers N` | Bounded Rayon completion of independent formula-reduct queries, including GPU residuals. Defaults to one worker. |
| `--batch-size N` | Bounds candidate batches. Logical completion scratch has its own explicit ceiling. |
| `--oracle auto` | Selects an exact supported check, including certified support checking where applicable. `closure` and `countermodel` make the procedure explicit. |

Metal has been physically qualified on an Apple M4 Pro for the recorded builds.
The static closure oracle uses a bounded ground graph; general formula execution
batches GPU propagation and exact CPU residual completion. Source loading,
parsing, materialization, candidate generation and objective work still run on
the host. Seven [physical lazy Metal tests](docs/verification/lazy-device-integration-20260907/physical/README.md)
pass for the prepared debug executables, including complete model comparisons,
catalog growth and bounded failure accounting. They do not establish release-binary
qualification or a speedup. Lazy general-formula execution remains an implementation
gap. Broader lazy Metal qualification is required before version 1.0. CUDA,
multi-GPU execution and neuromorphic backends are
future work. See [execution boundaries](docs/implementation.md) and the
[hardware evidence](docs/verification/metal-requalification-20260907/README.md).

## Status

The language target is clingo source and answer-set compatibility **excluding
theory atoms/terms, Python/Lua scripting, `#heuristic` and `#edge`**. The two
directives are deliberate project exclusions and receive explicit refusals.
Passing a selected corpus is not a
percentage of language compatibility. The source boundary uses the unchanged
themelios parser and owned program model; admission failures distinguish upstream
syntax, raising and evaluation errors from zetesis implementation refusals.

| Area | Implemented scope |
|---|---|
| Normal rules | Safe finite rules, constraints, default/double negation, strong negation with coherence, and relational lazy checking in the admitted normal-rule profile. |
| Formula rules | Bounded choices, signed singleton/disjunctive heads, finite rule/head pools, evaluated heads, scalar/range bindings, comparisons and admitted universal body conditionals. |
| Aggregates | Body count/sum/sum+ and complete-value min/max comparisons; independent assignments feeding scalar/tuple filters, scalar equalities, evaluated positive arguments and heads, and integer choice bounds; function-count heads with positive or default-negated eligibility and checked tuple/atom correspondence. |
| Logical values | Closed signed functions and tuples; finite construction from bound inputs; positive tuple/function patterns, including local conditional-consequent witnesses; evaluated positive arguments with independently bound inputs; evaluated already-safe negative arguments. |
| Objectives and observations | Admitted minimize/maximize/weak constraints; complete tuple keys and optimal ties; signature, term and conditional `#show`; `#defined`; original include bundles and constants. |
| Refusal boundaries | Cross-aggregate assignment dependencies, generated ranges and remaining conditional consumers, weighted/extremal function heads and nontrivial conditional disjuncts, objective-dependent disjunction/conditionals, broader directives and exact clingo undefined-arithmetic behavior remain incomplete. |

These rows summarize profiles; they are not a grammar specification. The
[source API guide](crates/zetesis-themelios/README.md) describes composition,
scope and resource limits. The [compatibility matrix](docs/verification/clingo-compatibility.md)
records the broader target. [Numeric semantics](docs/design/numeric-semantics.md)
explains endpoint guards and why an internal refusal does not by itself establish
a modeling error. Undefined or overflowing admitted arithmetic currently produces
an explicit refusal rather than reproducing all of clingo's simplifications.

The preceding execution tranche added [signed singleton heads](docs/verification/singleton-heads-20260907/README.md),
[constructor patterns](docs/verification/function-patterns-20260907/README.md) and
[evaluated positive arguments](docs/verification/positive-arguments-20260907/README.md).
For example, `q(X):-d(X),p(X+1).` consumes `X` from `d(X)` and retains the
matching `p` atom. Arithmetic does not infer an inverse binding.

The current source also adds [aggregate consumers](docs/verification/aggregate-consumers-20260907/README.md)
and [positive structured witnesses](docs/verification/structured-witnesses-20260907/README.md).
An independent assignment can feed `N>0` and `Y=N+1` before constructing `q(f(Y))`;
its proposed values still retain the original aggregate equality in the theory.
The consequent in `q:-p(f(X,_)):#true.` selects complete matching `p` atoms through
the existing transactional matcher. Local witnesses cannot establish outer or
condition safety, and matching possible support does not make an atom true.
Mixed evaluated/witness arguments, arithmetic inversion and negative anonymous
witnesses remain refused.

The [aggregate-bounds slice](docs/verification/aggregate-bounds-20260907/README.md)
adds evaluated positive arguments, independent integer choice-bound proposals and
positive count-head eligibility. For example,
`d(1).N{a;b}N:-N=#count{X:d(X)}.` retains the original equality around each
proposed bound, and `q(1).1#count{X:p(X):q(X)}1.` retains `q(1)` as a condition.
Neither proposed values nor possible-support membership establish truth.
The current [Lean audit](proofs/verification/consumers-memory-20260908/README.md)
checks 748 laws across 61 modules; Rust and WGSL correspondence remains unproved.

The **2026-09-08 consumer and memory tranche** adds completed aggregate-value
consumers in choice/count-head outer bodies, default/double-negated count-head
eligibility, compact private watch links and opt-in immutable-world source masks.
Local gates pass **1,779 workspace test/doc checks**, **300 CPU-only CLI checks**
and **59 external-oracle tests**; all 94 clean corpus and 24 selected upstream
cases still pass. Separate coverage profiles measure **91.1084% / 92.0460%**.
The [tranche record](docs/verification/consumers-memory-tranche-20260908/README.md)
keeps source, binary, audit and publication evidence separate.
The [September CI policy](docs/verification/local-macos-ci-20260908/README.md)
allows promotion using the completed local macOS gates while GitHub Actions is
paused through September 30. The new tranche has no executed hosted Linux result.

The [watch arrays](docs/verification/watch-storage-20260908/README.md) use half
their former element payload on the measured 64-bit build; ordered search traces
and counters are unchanged. The [source masks](docs/verification/source-masks-20260908/README.md)
omit joins with no common current world. Sparse controls save work; dense controls
pay overhead. Existing defaults are unchanged, and new masked physical execution
remains unqualified. Neither result is a whole-solver RSS or isolated runtime claim.

The **2026-09-07 aggregate-bounds and lazy Metal checkpoint** passes all **94 clean
non-clingcon kr-domains cases**, checking answer contracts, costs, counts and
optimum ties across 1,931 selected model occurrences. All **24 selected upstream
clingo assertions** pass with 73 full-model occurrences. Final local gates pass
**1,707 workspace test/doc checks** and **300 CPU-only CLI checks**, with explicit
external/device ignores retained. Corrected production line coverage is
**91.0705% workspace / 92.0126% CPU-only CLI** before the final status-styling
slice; both independent floors remain 91%. Hosted CI passed for that published revision.
The [checkpoint](docs/verification/aggregate-bounds-tranche-20260907/README.md)
records the rebuilt five release commands, source/binary identities, review
findings and limitations. Hosted results attach to their specific source
revision. The
[verification record](docs/verification/status.md) indexes historical results.

The [clean Lean build and audit](proofs/verification/aggregate-bounds-lazy-metal-20260907/README.md)
preserve the prior semantic sources and theorem/axiom records. These are checked
semantic laws, not a proof of the Rust/GPU implementation. Physical-device
qualification retains its separately dated executable identity.

The [updated end-to-end CPU comparison](docs/verification/cpu-refresh-20260908/README.md)
records the new solver binary, 21 alternating timed pairs per case, one worker
per solver and complete answer/optimal-tie enumeration. Native runs explicitly
request eager grounding; automatic oracle selection uses certified tight-support
checking for these inputs:

| Clean input | zetesis median | clingo 5.8.2 median | zetesis / clingo |
|---|---:|---:|---:|
| SEND + MORE = MONEY | 37.98 ms | 12.29 ms | 3.090 |
| Eight queens, variant 02 | 95.26 ms | 119.21 ms | 0.799 |
| Task allocation, variant 04 / larger mix | 126.98 ms | 182.30 ms | 0.697 |

All 150 comparison invocations and three separate diagnostic runs preserve the
complete reported-answer contracts. zetesis uses 20.1% less median wall time on
this queens input and 30.3% less on this task-allocation input; clingo remains
substantially faster on SEND. These are workload-specific CPU results, not a
general ranking or a controlled change from the previous release. Separate single
`--stats` observations put SEND grounding at **18.02 ms** and solving at
**15.22 ms**; these are not phase medians. Wall times include process startup,
output and capture with one-millisecond idle polling. Peak RSS is not measured.

The separate six-encoding campaign uses the same new binary and CPU/eager
protocol at size eight. All 306 observations pass, with the same 92 displayed
boards per encoding. Native/clingo median milliseconds are **9.21/6.17**,
**95.13/119.09**, **7.75/6.19**, **7.72/6.17**, **10.72/6.17** and **10.71/6.17**
for variants01–06. zetesis is faster on variant02; clingo is faster on the other
five. Both complete campaigns and previous results remain linked in the
[refresh record](docs/verification/cpu-refresh-20260908/README.md). No watch-only
speedup, GPU speedup or full-corpus performance ranking follows from these data.

The current [bounded expression-storage experiment](docs/verification/evaluation-scratch-20260907/README.md)
reduces median eager admission by 4.89% for SEND and 8.92% for queens02 in a
controlled comparison. It reuses empty storage within a join cursor and preserves
the checked arithmetic operations. These measurements exclude parsing and solving.
The end-to-end comparison above includes the change but does not isolate its effect.

The new [matched lazy CPU oracle experiment](docs/verification/ordered-joins-20260907/README.md)
measures 61.9–62.7% less time for sparse scalar joins at 256 rows and
68.3–71.1% less for eight-seed Rayon batches. Dense controls range from 4.0% less
to 1.4% more time. It excludes source preparation and outer search; the three
eager cases above do not exercise this optimization. Full samples and scopes
remain in the records.

The preceding [scalar ablation](docs/verification/scalar-evaluation-20260907/ablation.md)
and [end-to-end comparison](docs/verification/execution-performance-20260907/README.md)
retain their historical artifacts: a 61.5% isolated SEND admission reduction
and a separately measured 43.4% whole-process reduction. Physical GPU
microbenchmarks have mixed timing results; no new GPU speedup is claimed here.
The required [full corpus matrix](docs/design/corpus-performance.md) will compare
every non-clingcon kr-domains case across eager/lazy and CPU/Metal configurations
with clingo, retaining unsupported and incomplete cells. It has not yet been
collected; the current end-to-end campaigns cover the named CPU cases only.

## Libraries and mathematical specification

The project is library-first. Source programs, ground interpretations, checked
answer sets, objectives, completion and publication are distinct objects. Human
and JSON output are views of those objects. Public limits and typed failures are
part of each capability, not just command-line behavior.

| Start here | Responsibility |
|---|---|
| [zetesis-themelios](crates/zetesis-themelios/README.md) | Bounded source preparation, themelios analysis, relational admission and eager formula grounding, preserving origins. |
| [zetesis-domain](crates/zetesis-domain/README.md) | Conservative domain analysis over a borrowed themelios program, independent of solving. |
| [zetesis-core](crates/zetesis-core/src/lib.rs) / [zetesis-ferraris](crates/zetesis-ferraris/README.md) | Relational and formula semantics, interpretations, reducts and validated representations. |
| [zetesis-cpu](crates/zetesis-cpu/src/lib.rs) / [zetesis-wgpu](crates/zetesis-wgpu/README.md) | Closure execution and GPU primitives. |
| [zetesis-sat](crates/zetesis-sat/README.md) / [zetesis-objective](crates/zetesis-objective/README.md) | Native candidate/countermodel search and exact objective work. Boolean queries are internal machinery; stable acceptance belongs to the reduct composition. |
| [zetesis-cli](crates/zetesis-cli/README.md) | Prepared-input sessions, solve configuration, typed outcomes and output views, plus the process adapter. Extracting orchestration into a dedicated package remains planned. |
| [zetesis-validation](crates/zetesis-validation/README.md) / [zetesis-experiments](crates/zetesis-experiments/README.md) | Curated fixtures, reusable bounded process capture and reported-answer checks, external-oracle qualification and bounded measurements, separate from production acceptance. |

`prepare_formula` and `prepare_bundle_formula` expose analysis before ground
materialization. Consuming their receipts with `ground()` resumes the original
resource budget. Preparation is not a guarantee of successful grounding or
proof that a lazy strategy supports that source. Standalone ground/solve commands,
ASPIF interchange and Rust `@` functions remain design work.

The [design specification](docs/design/zetesis.md) gives the architecture.
The in-repository [Lean library](proofs/README.md) gives reusable mathematical
laws, with a [reading guide](proofs/guide/README.md), structured proof convention,
pinned toolchain and axiom/source audit. These are checked semantic contracts;
they do **not** certify the Rust compiler, search implementation or shaders.
Release work focuses on the [fundamentals enabling solver verification](docs/design/verified-solver.md),
with definitions and module boundaries that can fit a broader ASP theory library.
Comprehensive formalization of that broader theory is an adjacent long-term aim.
See the [theory-extension design](docs/design/theory-propagators/README.md)
and [neuromorphic feasibility assessment](docs/design/neuromorphic-feasibility-20260906.md)
for future integrations.
Selectable [Gelfond–Zhang aggregate semantics](docs/design/aggregate-semantics.md)
is a post-1.0 direction; the current aggregate semantics remains clingo/Ferraris.
[Brave and cautious consequences](docs/design/consequences.md) target version
1.1. Their design considers streaming and targeted restricted search
while keeping complete and partial conclusions distinct; no consequence-query
API or performance evidence is available yet.

## Build and check

New collaborators should start with [Contributing](CONTRIBUTING.md) and the
[development guide](docs/development.md): repository map, first edit, test
selection, qualification prerequisites and documentation responsibilities.

```sh
cargo build --locked -p zetesis-cli
cargo test --locked --workspace --all-features
./scripts/check.sh portable
```

The portable gate includes rustfmt, pedantic Clippy, strict rustdoc, both normal
and CPU-only CLI tests, Criterion correctness smokes and the remaining legacy
tooling tests. Rust property tests use proptest. Optional checks have explicit
prerequisites:

```sh
./scripts/check.sh oracle    # external clingo 5.8.2 on PATH
./scripts/check.sh proofs    # pinned Lean 4.33.1 through Elan
./scripts/check.sh coverage  # cargo-llvm-cov and llvm-tools
```

Workspace and CPU-only CLI coverage each retain an independent **91% line
floor**. Coverage does not measure assertion strength, Lean correspondence or
physical shader execution. GPU qualification is a separate test group on an
accessible device. The retained [CI workflow](.github/workflows/checks.yml) defines
portable gates on Linux/macOS, proofs and Linux coverage. Hosted execution is
temporarily disabled under the [local macOS policy](docs/verification/local-macos-ci-20260908/README.md).
A hosted runner does not establish NVIDIA or Metal hardware qualification.

The intended authored implementation is Rust, Lean and WGSL. Repository cleanup
is in progress. The first reusable Rust process/answer boundary now serves the
existing 94-case validator on Linux/macOS. It separates direct-child completion,
cleanup ownership and reported display evidence; it does not certify solver
correctness or hidden full models. The selected-upstream Rust campaign compares
typed native full models against all 24 pinned contracts and clingo, retaining
bounded failure evidence and primary-input seals. Its Python predecessor is
retired. Other Python qualification tools and C++ import provenance remain
until their replacements and independent evidence are verified.
Corpus expectation annotations are consumed only by separate validation tools;
they are ordinary comments to the solver and never guide its answers.
The [organization plan](docs/design/repository-organization.md) records the
migration without weakening gates or discarding evidence. Tooling follows the
same coding and review standards as the solver.

## Built on

[themelios](https://github.com/GregoryGelfond/themelios) supplies source identity,
parsing, programs and analysis at the estate's reviewed `87c11a3` pin.
[Rayon](https://github.com/rayon-rs/rayon) supplies CPU parallel execution;
[wgpu](https://github.com/gfx-rs/wgpu) supplies the GPU boundary.

zetesis is a separate experiment from apokrisis, and a member of the same estate
as themelios, keryx and morphe. The [standards alignment](docs/design/estate-alignment-20260907.md)
and [assurance audit](docs/verification/audit-contract.md) describe the quality
floor and the work still needed to reach the intended deployment standard.

## License

[MIT](LICENSE). Copyright © 2026 Gregory Gelfond.
