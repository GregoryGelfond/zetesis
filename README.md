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

Malformed source reports the original themelios parser messages with file,
line, column and marked source excerpts. Error headings and primary markers use
red; locations and explanatory context use italic gray. Included files retain
their own locations. Standard input uses the neutral `<input>` label. Diagnostics
go to stderr; syntax failure never becomes an `UNSATISFIABLE` result.

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

The [Linux/Vulkan qualification plan](docs/design/linux-vulkan-qualification.md)
covers a second reported Fedora 44/Radeon 780M machine and a single transferable
report archive. Vulkan execution is available through explicit selection, but
that machine has not yet been qualified; repeated benchmark interfaces still
need an explicit Vulkan route.

The [grounder comparison](docs/design/grounding-compared-with-clingo.md) explains
how zetesis's eager and lazy routes differ from clingo's grounder, including
their current language coverage and hardware boundaries.

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
the host. The current [17 physical Metal regressions](docs/verification/language-execution-tranche-20260908/README.md)
pass for frozen instrumented executables, covering lazy transport reuse, complete
closures, tight support, ordinary solving and admitted weighted heads. They retain
limits and output-failure accounting. These tests establish neither release
performance nor an RSS reduction.
Lazy general-formula execution remains an implementation
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
| Aggregates | Body count/sum/sum+ and complete-value min/max comparisons; acyclic dependent assignments feeding scalar/tuple filters, scalar equalities, evaluated positive arguments and heads, default/double-negated outer atoms and admitted projections, finite outer ranges, integer choice bounds, nonbinding aggregate guards and existing universal conditional scopes; count, signed numeric sum, nonnegative numeric sum+ and numeric min/max heads with retained eligibility and checked tuple/atom correspondence. |
| Logical values | Closed signed functions and tuples; finite construction from bound inputs; positive tuple/function patterns, including local conditional-consequent witnesses; evaluated positive arguments with independently bound inputs; evaluated already-safe negative arguments. |
| Objectives and observations | Admitted minimize/maximize/weak constraints; complete tuple keys and optimal ties; signature, term and conditional `#show`; `#defined`; original include bundles and constants; explicit parameter-free `#program base` sections. |
| Refusal boundaries | Cyclic or self-dependent assignment generators; unsupported local conditional generators; negative sum+ head weights, nonnumeric measured heads and broader tuple aliases; nontrivial conditional disjuncts; unsupported objective-dependent producers; broader directives and exact clingo undefined-arithmetic behavior remain incomplete. |

These rows summarize profiles; they are not a grammar specification. The
[source API guide](crates/zetesis-themelios/README.md) describes composition,
scope and resource limits. The [compatibility matrix](docs/verification/clingo-compatibility.md)
records the broader target. [Numeric semantics](docs/design/numeric-semantics.md)
explains endpoint guards and why an internal refusal does not by itself establish
a modeling error. Undefined or overflowing admitted arithmetic currently produces
an explicit refusal rather than reproducing all of clingo's simplifications.

Completed outer values feed [negative consumers](docs/verification/outer-negative-consumers-20260908/README.md)
and [finite dependent ranges](docs/verification/outer-ranges-20260908/README.md)
in ordinary, choice and checked count-head bodies. For example,
`{p(0)}.q(N):-N=#count{},not p(N).` has answer sets `{p(0)}` and `{q(0)}`:
the count supplies an argument, while the original negative atom decides whether
the rule applies. Similarly, `{d}.q(K):-N=#count{1:d},K=1..N.` has the empty
answer set and `{d,q(1)}`. Each generated row retains the original aggregate
equality and rule activation; a proposed value never certifies aggregate truth.

Completed values also feed the existing universal conditional scopes, with every
original implication retained. The [conditional-consumer record](docs/verification/conditional-consumers-20260908/README.md)
compares 33 original sources, 146 complete clingo models and 9,876 original/frozen
interpretation pairs. Explicit parameter-free `#program base` declarations now
work across [source and bundle boundaries](docs/verification/base-sections-20260908/README.md);
named or parameterized sections remain located implementation refusals.

[Numeric sum heads](docs/verification/weighted-heads-20260908/README.md) now keep
head permission separate from the numeric bound, including zero-weight heads.
The scoped checks retain 27 originals, 43 complete clingo models and 846 arbitrary
frozen interpretation pairs, plus generated arithmetic properties. Negative
`#sum+` head weights remain an internal limitation with a recorded reference
discrepancy; they are not labeled modeling errors.

The [last completed integrated qualification](docs/verification/language-execution-tranche-20260908/README.md)
passes **2,178 workspace test/doc checks**, **339 CPU-only CLI checks** and
**65 external-oracle tests**. Both unchanged 91% coverage floors pass:
**91.9033% workspace with the four designated Metal tests / 93.1998% independent
CPU-only CLI**. The portable workspace stage remains visible at 90.6927%.
The frozen release passes **94 clean kr-domains cases** and **24 selected
upstream cases**, preserving their recorded display/cost and full-model contracts
respectively. The [current CPU and Metal measurements](docs/verification/language-execution-tranche-20260908/timing/README.md)
retain complete populations and timing limitations. The new transport comparison
has mixed results; it does not establish a general GPU speedup.
The [current Lean audit](proofs/verification/aggregate-primitives-20260908/README.md)
checks **812 laws across 71 modules**; Rust and WGSL correspondence remains unproved.

The next tranche has integrated [completed-value aggregate guards](docs/verification/nonbinding-guards-20260908/README.md):
46 originals matched 88 complete clingo models, with 7,312 frozen interpretation
pairs checked separately. [Numeric min/max heads](docs/verification/extrema-heads-20260908/README.md)
add 51 originals matching 94 complete clingo models and 5,439 frozen pairs;
guards retain ASP term order and genuine empty extrema. The first tuple values
remain numeric under the documented endpoint profile. The
[partition experiment](docs/design/partition-consequences.md)
derives candidate bounds from supplied premises; automatic source recognition
and performance qualification remain open. The full tranche is still in development.

A standalone [tight-support Metal experiment](docs/design/metal-tight-support.md)
now applies the CPU class certificate through a bounded GPU checker. Its matched
scalar/Rayon/fresh-Metal/resident-Metal harness preserves exact CPU completion of
residuals. Four [physical Metal tests](docs/verification/tight-metal-experiment-20260908/README.md)
pass on Apple M4 Pro. [Matched measurements](docs/verification/tight-metal-experiment-20260908/measurements/README.md)
now show that residency helps, but this primitive remains slower than scalar and
Rayon checking in all 18 tested case medians. Those measurements retain their
earlier experimental binary identity; ordinary backend selection is unchanged.
The [next tranche proposal](docs/design/next-language-and-execution-tranche.md)
prioritizes the remaining admitted-language gaps, with native aggregate
primitives, bounded CPU/GPU experiments, Linux qualification and a broader
foundation research survey alongside them.

The [September CI policy](docs/verification/local-macos-ci-20260908/README.md)
uses local macOS qualification while GitHub Actions is paused through September
30. This checkpoint has no hosted Linux result. Historical qualifications and
source-specific limitations remain in the [verification index](docs/verification/status.md).

Relational lazy source joins offer an opt-in world-membership filter, which omits
joins with no common current candidate world. Sparse controls save work; dense
controls pay overhead. [Reusable join frames](docs/verification/join-workspace-20260908/README.md)
now reduce repeated preparation while membership is rebuilt every round and
retained storage stays charged. Defaults remain unchanged. The updated masked
path now passes [physical Metal qualification](docs/verification/consumer-execution-tranche-20260908/physical/README.md)
for the recorded 33-occurrence fixture and chunk sizes one and seven. No GPU
speedup or process-RSS claim follows from these work and transfer controls.

The [current CPU comparison](docs/verification/language-execution-tranche-20260908/timing/README.md)
uses 21 alternating timed pairs per case, one worker per solver and complete
answer/optimal-tie enumeration. Native runs explicitly request eager grounding;
automatic oracle selection uses certified tight-support checking on these inputs:

| Clean input | zetesis median | clingo 5.8.2 median | zetesis / clingo |
|---|---:|---:|---:|
| SEND + MORE = MONEY | 38.08 ms | 12.86 ms | 2.960 |
| Eight queens, variant 02 | 89.57 ms | 119.52 ms | 0.749 |
| Task allocation, variant 04 / larger mix | 120.70 ms | 179.94 ms | 0.671 |

All 153 baseline and 306 separate six-queens observations pass. Zetesis has the
lower median on this queens input and task-allocation case. Across all six
queens encodings, clingo is faster on the other five. Every encoding retains
all 92 displayed boards at size eight; hidden atoms differ. Full per-encoding
results and separate diagnostic counters are retained in the timing record.
The [queens analysis](docs/verification/language-execution-tranche-20260908/analysis/queens.md)
identifies candidate generation and representation size as general optimization
targets. The [graph and allocation analysis](docs/verification/language-execution-tranche-20260908/analysis/domains.md)
separates current correctness evidence from the earlier full-corpus timing matrix.

These are descriptive CPU comparisons, not an isolated optimization effect or
a general solver ranking. Preparation I/O may have overlapped SEND or baseline
measurement; the coordination audit preserves that limitation. Process timers
include startup, capture and each producer's different output format. A separate
33-admission SEND study retains a median of **17.70 ms** for admission without
internal observers;
its detailed mode still attributes most admission work to rule instantiation.
Peak RSS and lazy/GPU timings are not part of this whole-solve population.

An earlier [controlled propagation comparison](docs/verification/binary-propagation-20260908/README.md)
isolates omission of empty binary-clause replacement scans. Across 42 timed
native observations per variant, queens02 falls from **98.26 to 92.80 ms** and
task allocation from **130.05 to 126.38 ms**; SEND is flat at **39.54 / 39.60 ms**.
All 612 solve observations pass. The ABBA order, full distributions and reference
drift are retained; this establishes neither a general speedup nor a memory or
GPU improvement. These source-controlled populations are distinct from the
current workload comparison and must not be combined as one timing series.

The [instrumented M4 Pro corpus matrix](docs/verification/dependencies-measurement-tranche-20260908/physical/README.md)
now records explicit CPU/Metal and eager/lazy requests over all 94 clean examples.
All eager CPU and Metal observations match clingo's complete selected-answer,
cost and optimum-tie contracts. Both lazy profiles refuse all 94 formula inputs;
those positions have no timed population. Metal/eager is slower than CPU/eager
on all 94 inputs here: the median per-case ratio is 3.020. Device setup and
different oracle selections contribute; native full-model JSON and statistics
also make this a different output protocol from the earlier CPU comparison.
[All six queens encodings and all other inputs](docs/verification/dependencies-measurement-tranche-20260908/physical/corpus-timing.md)
retain their individual timings and scopes.

The separate `zetesis-bench lazy` compares identical frozen candidate batches
across scalar, independent Rayon, shared CPU and Metal source rounds. The latest
[audited old/new/new/old comparison](docs/verification/language-execution-tranche-20260908/timing/metal/review/README.md)
retains all 4,320 samples and 231,840 checked occurrences. Of 24 Metal comparisons,
four improve in both pairs, four regress and sixteen reverse direction. The new
transport reuses storage on 240 of 3,120 dispatches; this modest reuse does not
establish a general speedup or lower process RSS.

For dense width-8/128-world batches, the new per-world Metal route takes a pooled
median **6.944 ms**, independent Rayon **14.921 ms**, and shared CPU Union
**4.808 ms**. The fastest observed CPU route beats the fastest Metal route in all
12 cases. Complete source/reduct-call timers exclude parsing, outer search,
objectives and device setup; these synthetic repeated-candidate populations are
separate from ordinary whole solves. See the [matrix contract](crates/zetesis-validation/README.md)
and [lazy experiment](crates/zetesis-experiments/README.md) for output, timing and
resource scopes. Compact saved reports preserve every captured byte; their
compression does not demonstrate lower solver memory use.

The [compact-trail experiment](docs/verification/trail-storage-20260908/README.md)
was declined: its small storage saving did not establish a reliable runtime
benefit across 612 complete comparisons. The production trail remains unchanged;
the source patch, all observations and experimental proof evidence are retained.

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

Earlier [scalar arithmetic measurements](docs/verification/scalar-evaluation-20260907/ablation.md)
and [whole-process results](docs/verification/execution-performance-20260907/README.md)
retain their original sources and scope. These physical oracle measurements do
not establish a faster ordinary solver or an automatic GPU crossover.
The broader [corpus performance protocol](docs/design/corpus-performance.md)
still requires additional repetitions, worker configurations, parameterized
sizes and memory measurements. The first instrumented matrix covers every
original input and requested route, retaining unsupported cells explicitly.

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
./scripts/check.sh coverage --metal # optional actual tight-oracle Metal tests
```

Workspace and CPU-only CLI coverage each retain an independent **91% line
floor**. The explicit Metal mode retains portable-stage coverage, then adds four
physical tight-oracle tests within the workspace profile; it never merges the
CPU-only CLI profile. Coverage does not measure assertion strength, Lean
correspondence or shader execution. Other GPU paths require their own physical
test groups. The retained [CI workflow](.github/workflows/checks.yml) defines
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
