# Library-first execution and interchange — 7 September 2026

This bounded architectural audit examines commit `b760db2` and the initial
telemetry/frontend observer design as inspected at that time. It establishes no
new solver, build or benchmark results. Reusable libraries own capabilities and
the CLI composes them. The audit also examines the themelios, keryx and morphe
library surfaces and their relevant designs as inspected at that time; this is
a bounded architectural sample, not a complete implementation or security audit
of those repositories.
The ASPIF section below includes a read-only protocol follow-up; its proposed
record profile is not an implemented capability.

**Implementation follow-up.** The bounded typed model view, `--json` document
adapter and separate eager grounding/solving measurements were subsequently
delivered in the [library boundaries checkpoint](../verification/estate-boundaries-20260907/README.md).
The proposal below remains a record of the design at inspection time. The later
[API hardening checkpoint](api-hardening-20260907.md) implements coherent prepared
inputs, writer-free sessions and finalized semantic outcomes over shared ordinary
execution loops. The session remains in the existing CLI library package; dependency
extraction and ASPIF interchange remain unfinished.

The existing semantic kernels are already useful libraries. The highest-priority
gap is the composition layer: the ordinary source-to-optimal-model path can only
be reused through a CLI-shaped configuration and byte writers. Adding a renamed
crate containing that same interface would not close the gap.

## Concrete estate precedents

The following observations are from actual checked-out source, with design intent
identified separately from implemented capability. No sibling repository is changed.

| System | Implemented library and view boundary | Application to zetesis |
| --- | --- | --- |
| themelios | `crates/themelios-base/src/diagnostic.rs` owns typed `Diagnostic` with stable identity, severity and located labels; `view.rs` derives human and typed editor views over the model and a `Sources` environment. `themelios-program/examples/first_solve.rs` constructs the same `Program` through two typed doors; its comment explicitly says ground/read-back awaits the solve tier. | Reuse the logical vocabulary and source evidence. Expose result data before views; do not obtain structured answers by parsing human output or claim a designed solver already exists. |
| keryx | `keryx-core::codec::Codec` offers typed `shred`/`reassemble`; `Facts::symbols` returns themelios `Symbol` values and `Facts::render` is a derived view. `diagnostics.rs` owns typed `Diagnostic`/`Diagnostics`, `Display` and `Diagnostics::wire`; `keryx-cli::render` chooses the view and writes it. CLI `ShapeArg` converts into core `Shape`; the library type does not derive clap traits. | A model's machine view can live beside its typed model in an existing module. Source/wire adapters are separate from the semantic value. CLI testability alone is not the library boundary. |
| morphe | `morphe::{format,format_parsed}` share one pure pipeline and return `Formatted` plus typed certificate/refusal data. `FormatOptions` and injected `EmbeddedFormatter` are library APIs; optional concrete backends are feature-gated. CLI `DialectArg` maps into the library `Dialect`, configuration/filesystem handling stays in the CLI, and typed notes are rendered through themelios views. | Provide source and already-admitted doors without duplicated execution or mandatory reparsing. Keep library configuration free of clap and select injected backend mechanisms at composition boundaries. |

The pins are intentionally independent: the inspected keryx manifest uses themelios
`86c7dfb`, morphe uses `87c11a3`, and zetesis pins the full `87c11a3...` revision.
This is an API design comparison, not evidence that types from independently pinned
builds can be linked or exchanged without an explicit dependency alignment.

`themelios/docs/design/solve.md` is explicitly **draft, pre-implementation**. Its
§1.1 defines solve as a behavioral target contract over the existing `Program`,
with a ground representation as a named capability rather than a second central
source representation. §1.3 requires typed answers and derived human/machine
views. §2.2 locates closely related `contract`, `outcome`, `session`, `extend`,
`bridge` and `conformance` modules in one capability crate; it rejects an automatic
crate split for every conceptual boundary. §10.4 commits the optional ground-program
observer capability, including `Origin` evidence, while keeping it outside the
mandatory lean contract. These are the appropriate alignment targets for a native
zetesis adapter when that tier is implemented. They are not current callable APIs.

The concrete immediate placement recommendation is therefore **modules first**:
typed observation/model view helpers and pure bounded views in the existing
`zetesis-themelios::observation` capability; CLI format/document/process adaptation
in `zetesis-cli::output`. A JSON crate or a crate containing only a model callback
trait has no present justification. A later runtime capability, if a new crate is
needed to provide a clap-free dependency surface, should own its contract, outcome,
session and views as modules together. That crate's purpose is actual source/native
solve composition and a separately consumable dependency boundary, not a rename
of the current CLI. The proposed telemetry crate has a different, concrete reason:
an independent std-only recorder shared by frontend composition and solve clients.

## The logic programmer's register and its proof correspondence

The source layer provides more than shared data types. `themelios_program` calls
its owned representation the logician's program and exposes construction,
substitution, unification and transformation as operations on that object.
`themelios_analysis::Analysis::of` computes structural facts once; callers inspect
constructs, dependencies, safety and literature-defined classes with witnesses.
It is pure and total over its program value, including a recovered value. That
last fact does not license zetesis to solve a program with admission errors.

Carry this register into the solve capability. Its public questions concern
consistency, answer sets, consequences, assumptions and optima. Typed backend
policy, counters and execution plans are useful advanced capabilities, but they
must not define what an answer set is or force every consumer to understand a
candidate queue. Display choice belongs to a view; selecting `#show` symbols does
not select a different stable-model semantics. The distinction is mathematical,
not a cosmetic rename of internal engine operations.

| Semantic object or obligation | Existing Lean anchor or remaining obligation |
| --- | --- |
| An interpretation satisfies the original finite theory | `Ferraris.Models M T`; this alone does not establish stability. |
| An interpretation is an answer set | `Ferraris.Stable M T` and `stable_iff_minimal_reduct`; a proper subset may not model its frozen reduct. |
| A certified specialized check preserves membership | `CertifiedExecution.completed_membership_exact`, using an explicit sound verdict and exact residual-oracle premise. |
| Interruption supplies no accepted membership result | `CertifiedExecution.interruption_cannot_accept`; a stopped attempt is distinct from false membership. |
| Candidate restrictions preserve the question about the original theory | `CertifiedExecution.restricted_result_original`; the region filters candidates while `T` remains fixed. |
| A source program and its admitted native representation ask the same question | A separate bounded source/lowering correspondence; the finite-theory theorems alone do not prove it. |
| The API returns exactly the requested complete models or a proved optimum | Compose membership, coverage and objective laws with the actual session's limits and publication contract; not implied by membership soundness alone. |
| CPU, Rayon and GPU execute the same operation | A representation/execution refinement plus qualification evidence; no shader correctness follows from a semantic theorem. |

This alignment makes theorem statements more direct by sharing concepts and
invariants. It does not make all proofs easy, and identical names are no evidence
of refinement. In particular, today's Rust `Model` is an interpretation container;
its name does not itself carry a stability proof. A future `AnswerSet` result must
be produced through the checked membership boundary and retain the program/version
association needed by its contract. Views must not manufacture that result type.
Likewise, a countermodel is relative to a particular frozen candidate reduct;
it is not automatically a counterexample to a different candidate's stability.

Follow a denotation-first proof structure: state the logical operation, its
applicability and completeness conditions, then the representation correspondence,
then the implementation schedule. An embedding API should make those conditions
visible through types or documented validated constructors. The Lean development
should use corresponding predicates rather than mirror CLI branches, queue
indices or GPU buffer names. Such execution details have their own refinement
lemmas beneath the semantic contract. The [structured proof convention](../../proofs/STYLE.md)
and [library reading guide](../../proofs/guide/README.md) carry this register into
the mathematical library, starting with one statement-preserving pilot. This is consistent with the estate's
lean contract and ergonomic facade, and with zetesis's compositional oracle.

## Existing boundaries worth preserving

| Boundary | Public evidence at `b760db2` | Consequence |
| --- | --- | --- |
| Native relational vocabulary | `zetesis-core/src/lib.rs`, `ground.rs`, `structured.rs`: immutable `Program`, `Model`, program-bound `Seed`, `GroundProgram::compile`, bounded structural `Value` | Callers construct and inspect logical data without a parser, process or output writer. Eager compilation is explicit. |
| Source integration | `zetesis-themelios/src/lib.rs`, `bundle.rs`, `formula.rs`: source/bundle admission, typed limits/refusals, `AdmittedFormula::{theory,atoms,objectives,metadata,analyzed_program}` | Parsing, raising, normalized analysis and original provenance remain in the existing adapter over pinned themelios `87c11a3f2b72b81a12fd53226941fdf95e7294d3`; no second parser is needed. This production crate depends on neither clap nor SAT. |
| Formula semantics | `zetesis-ferraris/src/{lib,theory,oracle,normal}.rs`: `Theory`, `Interpretation`, original/frozen-reduct evaluation, subset checking, aggregate constructors, normal-program conversion | Finite formula construction and reduct semantics are already independently usable. Interpretation identity is tied to the immutable theory. |
| Search and execution primitives | `zetesis-cpu` closure/candidate/control APIs; `zetesis-sat::StableModels`, completion executor, typed limits/statistics, `SearchPhaseTimings`; `zetesis-wgpu` device/oracle APIs | Exact search, cancellation and batched membership are reusable. Candidate restrictions need not mutate the original theory. Actual GPU selection remains distinct from CPU residual work. |
| Objectives and observations | `zetesis-objective::evaluate`, typed `Score`; `ObservationProgram::{evaluate,render}`, `OutputSelection` | Scoring reads a verified full model; it supplies no support. Typed observation symbols already exist independently of human rendering. Display projection never changes model identity. |

`GroundProgram` is the normal closure specialization, with a single optional head
and separate positive/frozen true/frozen false guards. It is not the universal
ground representation for disjunction, general formulas and all objective data.
`AdmittedFormula` is a useful standalone grounding result today, but its objectives
and observations remain lifted plans and its owned compiled payload is private.

## The concrete composition gap

`zetesis-cli::run*` accepts `Options`, `Write` sinks and `Control`, returning `Report`
or `RunFailure`. `Options` derives clap `Parser` and combines input filenames,
subcommands, display count, hardware policy, admission limits and search limits.
`Backend`, `Grounder` and `Oracle` also derive clap traits. Injecting a writer makes
the CLI testable, but callers still cannot consume the ordinary algorithm as a
typed model stream.

The following semantic orchestration currently remains private in that crate:

- `admission.rs` chooses closure admission and the bounded formula fallback.
- `engine.rs`, `formula_execution.rs` and `formula_queue.rs` select actual backend,
  own completion queues, retain pending/committed batch accounting and perform
  exact fallback. They consume CLI `Options`; several also emit diagnostics.
- `optimization.rs::Incumbents` owns cumulative objective work and bounded tied
  model retention. `objective_bounds.rs` composes optional candidate restrictions.
  These are not available from the public objective evaluator alone.
- `countermodel.rs::FormulaRun` combines membership, scoring and publication;
  `Display::write` and `driver::write_model/finish` bind that control flow to human
  records. `Report.models` counts accepted output records, while `PartialReport`
  separately retains verified models and publication failures.

The failure distinction is valuable and must survive extraction: a source/setup
failure may have attempted timing but no semantic report; a membership batch can
verify more models than were consumed; an optimal incumbent can exist without a
proved optimum; a failed output write does not invalidate already verified models.
The public detailed API preserves this evidence; its convenience wrapper explicitly
discards it. A new typed API should preserve it by default.

## Immediate timing seam

The `b760db2` public `PhaseTimings` lives in the CLI; its recorder is private and
enabled by `Options.stats`. The driver attaches a typed snapshot and also writes
the text protocol. Lower-level SAT phase timing already has a direct library API.

The inspected timing design takes an additive direction:

- A clock-free `zetesis_themelios::GroundingObserver` and optional observed string
  and bundle formula APIs wrap exactly `formula_ground::ground`. An RAII exit
  runs when that action returns an error or unwinds; observers are documented not
  to panic. Ordinary admission has no observer or clock dependency.
- A small std-only `zetesis-telemetry` owns `StageRecorder`, guards,
  `StageTimings`, `StageMeasurement`, `SolveStage` and `GroundingMode`. It imports
  no CLI, frontend, SAT, backend, writer or semantic completion type. Initial code
  uses fixed storage, skips clock reads when disabled, suspends parent stages
  under children, and snapshots active prefixes without mutating the live prefix.
- CLI adapters connect those public APIs and render protocol lines. Validation's
  retained JSON/protocol parser consumes the fixed schema independently of solver
  internals. Its serializable evidence DTO is not the producer's runtime clock.

This capability boundary supports stage statistics before full driver extraction. Keep recorder lifetime caller-defined.
`driver_elapsed` must mean the documented recorder interval, not an assumption
that the caller is a CLI. The standalone library caller must be able to collect
the same typed snapshot without any text output.

Guard tests should cover repeated active snapshots, nesting including the same
stage, out-of-order drops, failures/unwind, unavailable versus measured zero,
overflow and both transitions to mixed eager/lazy mode. Numeric unattributed time
requires an exact checked exclusive partition. Completeness here means counter
and duration integrity, never exhaustive search. Lazy grounding remains unavailable
as a separate duration; mixed mode retains only measured eager intervals. Do not
sum worker CPU intervals, nested phases or GPU kernel guesses into host elapsed.
Coarse stage schema 1 and detailed phase schema 1/2 remain distinct protocols.

## Minimal coherent reusable runtime

Introduce a real composition API in dependency order, using the existing kernels.
This is an engine capability that can implement themelios's eventual shared solve
contract, not a competing estate-wide session or callback framework. Preserve the
[existing integration direction](themelios-solve-integration.md); adapt final public
signatures to the implemented shared tier when it exists:

1. Define writer-free `SolveConfig` and typed execution policy/limits. A separate
   CLI adapter maps clap options into them. Filename loading, subcommands, output
   format and process exit codes stay outside this solve configuration. Hardware
   discovery/fallback produces typed events; display wording belongs to adapters.
2. Accept an already admitted native input as well as a source-admission service.
   Avoid parsing and grounding again when an application has already admitted
   the same input. Preserve the aligned atom table, original immutable theory,
   objective/observation plans and optional provenance; validate their association
   at construction instead of exporting an unchecked bag of slices.
3. Expose a bounded `SolveSession`/iterator or callback-based execution with typed
   model events and a terminal outcome. Separate verified membership, incumbent
   updates, publishable solutions, exhaustive/partial coverage and interruption.
   Full models and scores remain semantic data; sink acknowledgement records
   delivery, not durable storage. A fallible sink must retain the last trustworthy
   outcome and original failure without inventing completion.
4. Move algorithm ownership one vertical slice at a time: formula CPU membership,
   cumulative objective evaluation, tie retention and restriction feedback first;
   then closure and batch/GPU composition through the same interface. Existing CLI
   wrappers call this implementation. Do not maintain a second semantic driver.
   The incumbent policy may live in the composition library or a suitably generic
   objective component; it must not depend on clap, rendering or filenames.
5. Keep timing in reusable stage owners and the composition session. The runtime
   consumes frontend/SAT measurements and telemetry; a frontend never imports the
   runtime or CLI. A small telemetry crate is justified by actual shared use;
   core should not become a repository for process/reporting utilities.

The first acceptance example must solve a typed admitted program through this API
with no `zetesis-cli`, clap, `Write`, subprocess or human parser dependency. Compare
the same implementation's CLI output against it for original full model identity,
all optimal ties/cost priorities, limits, cancellation and failure evidence. Keep
the established certificate budget, immutable-reduct, batch completion and partial
publication regressions. No algorithm or language expansion is required for this
extraction.

## Standalone grounder, native artifact and ASPIF boundary

Provide three independently usable library operations: source to admitted native
ground artifact, native artifact to exact solve session, and interchange import /
export around that artifact. Ground-only operation must invoke no solver. Solving
an imported/native artifact must require no source parser. Source provenance is an
optional sidecar, not a prerequisite for semantic execution.

Keep this a named capability under the same behavioral solve contract described
by themelios. An ordinary lazy solve need not first materialize a complete ground
artifact. An explicit standalone ground request must either produce the declared
bounded ground artifact or refuse; a source template is not a completed ground
result. Source-origin evidence is carried when available, with imported/constructed
origins identified honestly.

The public native artifact needs a validated complete solver contract: atom identity
and optional symbol mapping, the ground logical representation, objectives and
priority presence, output mapping, and declared supported features. Imported opaque
atom identifiers must not be mistaken for invented source predicate names. Current
lifted objective/observation plans can remain an explicit native capability, but a
claim of fully ground interchange requires their bounded materialization or an
explicit refusal. A `Theory` alone omits these channels.

Use themelios integration for shared logical terms, signatures, source identity and
any existing upstream interchange facilities confirmed by API inspection. Keep native artifact validation and codecs reusable without the CLI.
Do not implement another textual ASP parser or export by printing source and
regrounding it.

ASPIF import/export must have a declared record/capability profile and a fallible
bounded conversion. Unsupported semantic records are refused, never dropped. The
initial profile should be chosen only after checking the official protocol and the
pinned dependency's facilities. This audit does not assert which ASPIF records that
dependency supports. Export must preflight atoms, rules, terms, objective entries,
auxiliaries, bytes and work before retaining expanded data.

“Lossy” can describe original source spans, spelling, rule grouping or internal
representation. It must not describe changed stable models, optimum/cost vectors,
visible output or supported assumptions. Any auxiliary atom encoding requires a
stable-model correspondence with a specified projection and multiplicity, including
frozen reducts; classical Boolean equivalence alone is insufficient. Round-trip
tests compare the complete admitted semantic contract and typed model/cost results,
with empty programs, constraints, signed atoms, choices/disjunction, objectives and
output conditions included when supported. Semantic preservation laws should state
original-model and frozen-reduct correspondence, auxiliary projection properties,
and objective/output preservation separately. Lean laws specify these translations;
passing them alone would not verify the Rust codec.

## ASPIF protocol follow-up and proposed first profile

Read-only inspection of themelios confirms that `solve.md` §§10.2–10.4 already
specifies an ASPIF input door, a typed ground sink with multiple implementations,
and an engine-free ground-program observer. These are draft contracts. The
currently inspected syntax implementation recognizes an ASPIF header and emits
`syntax::aspif-input`; it does not parse ASPIF into ground records. A separate
interchange decoder is appropriate and does not duplicate the ASP source parser.

Protocol evidence is the locally available clingo source at
`archeion/vendor/clingo/clasp/libpotassco/src/aspif.cpp`, its
`potassco/aspif.h` interface, and `tests/test_aspif.cpp`. Potassco's
[ground program backend documentation](https://potassco.org/clingo/c-api/5.8/group__ProgramBuilder.html)
also exposes typed rule, weight-rule, objective and control operations. These
sources describe the boundary; they do not establish a zetesis implementation.

The following is a proposed, deliberately bounded first interchange profile:

| ASPIF channel | Initial conversion obligation |
| --- | --- |
| Header and steps | Declare accepted version and exactly one completed nonincremental step. Refuse incremental input until versioned sessions implement its semantics; reject trailing records and incomplete framing. |
| Disjunctive (including singleton normal) and choice heads with normal or weight bodies | Preserve distinct opaque atom IDs, signed body literals, bounds and multiplicity according to the record semantics. Empty heads remain constraints. Refuse unsupported numeric ranges and expansions before publication. |
| Minimize records | Preserve signed weights and every priority slot, including zero costs. ASPIF is already lowered: do not apply source aggregate tuple deduplication to weighted records. |
| Output records | Keep their conditional strings as an output mapping, separate from semantic atom identity. A foreign ground program need not provide symbolic names for all atoms. |
| Project, external, assume, edge | Explicit refusal in the first profile unless an exact documented implementation is included. These are not harmless comments. Source-language support does not automatically imply equivalent ground-record support. |
| Heuristic records | Initially refuse; a later policy may explicitly accept and report disregarded search hints after proving they do not change the declared result contract. |
| Theory records | Explicit unsupported capability until the shared propagator/theory contract is implemented. |
| Comments | May discard with the declared representation/provenance loss; never interpret them as instructions. |

This profile is an engineering starting point, not a promise of arbitrary clingo
ASPIF compatibility. Match integer domains, byte-counted output strings and literal
zero/reserved-ID rules against the protocol. Do not infer literal semantics from
printed atom spelling: an ASPIF negative body literal is default negation over an
atom ID; a strongly negated predicate is a distinct source atom. Preserve any
coherence constraints emitted by the grounder without inventing identities from
output strings.

Export of the current general Ferraris formula DAG is a semantic lowering,
not a serializer. It may require auxiliary atoms with unique corresponding
extensions and frozen-reduct preservation. A classical Tseitin encoding or
unconditional disjunction shift is not an adequate stable-model argument. Native
lifted objectives and observations need finite checked lowering before this export
profile applies. Preflight the entire semantic conversion before exposing a
completed artifact; bounded streaming may still fail at the transport boundary.

The initial differential acceptance matrix should isolate the components:

- Source → zetesis eager ground artifact → ASPIF → external clingo/clasp.
- Source → external gringo/clingo ground output → ASPIF → zetesis solve.
- Native artifact → ASPIF → native artifact, compared through full model identity,
  projected auxiliary correspondence, objective vectors and output mappings.
- Every unsupported record and resource limit returns a typed refusal; neither
  a truncated step nor partial lazy materialization becomes an empty program.

External Potassco tools belong only to this qualification matrix. Ordinary ground
and solve libraries remain independent of those executables and their libraries.

## Bounded JSON delivery slice

The requested `--json` should select a renderer; clean human output stays the
default. Do not implement JSON by capturing and reparsing `Answer:` lines. Existing
`Model`, `Score`, `OutputSelection` and `ObservationProgram::evaluate` supply the
typed inputs already.

For the immediate slice, expose the borrowed model/view record and view functions
from an `observation` submodule of the existing frontend library. That layer already
depends on core and objective and owns the display semantics; it needs no clap,
global streams or process policy. The CLI document renderer can initially consume
its current typed `Report`/`RunFailure` from a dedicated output module. State this
remaining outcome/config coupling explicitly: it is an interim adapter, not a
claim that the entire solve API is now independent of the CLI. Move the session's
outcome and views together when the reusable runtime capability is extracted.

The smallest useful slice adds a reusable typed model-event/sink boundary and
routes every publication site through it: closure `driver::write_model`, formula
`Display::write`, and final `Incumbents::write`. A borrowed full model, optional
score and typed shown atom/term channels are enough; no new all-model vector is
needed. Human and JSON renderers consume this same boundary. Equal rendered
symbols from distinct channels and distinct full models with identical projections
must retain their current multiplicities.

Prefer a versioned streaming JSON document with a `models` array and terminal
typed outcome. Buffer at most one bounded complete record before external emission,
with checked JSON escaping/size accounting and explicit record limits. Existing
bounded incumbent retention remains responsible for optimization; do not publish
tentative incumbents as proved optimal ties. Define whether full semantic atoms are
included in addition to shown channels, then expose them as separate fields so
consumers cannot confuse display with identity. Costs retain priority and value,
including present zero/empty slots according to the typed score contract.

The terminal object needs coverage, termination reason, model/verified/publication
counts as applicable, objective evidence and optional typed statistics/timings.
Represent unavailable data as unavailable, not zero. Semantic/input failure may
finish a valid error document if the sink still works. Transport failure can leave
a truncated document; retain partial evidence in the returned library error and
never claim a completed JSON publication. Increment the published count only after
the record write is accepted. Preserve the existing process exit policy.

Implementation scope: shared publication hooks, typed sink/record module, human and
JSON adapters, option/process selection and focused tests. Timing hooks must retain
their existing contracts. Test plain/structured/string/signed/sentinel
values, hidden display duplicates, term-channel duplicates, empty output/UNSAT,
objectives and all ties, requested count, interruption, byte ceilings and injected
writer failure. A programmatic sink test should demonstrate reuse without a human
renderer. ASPIF and the whole runtime extraction are follow-on work; the initial
JSON seam must make those future consumers easier rather than introduce another
private CLI-specific model representation.

## Measurement and scope guardrails

The retained ordinary-performance harness intentionally measures complete CLI
invocations on original inputs, including startup, admission, route selection,
optimization and output. That is the correct evidence for ordinary CLI latency;
it is not interchangeable with a preadmitted-theory loop. Keep all existing source
and binary seals, matching worker conditions, rotated samples, objective contracts
and displayed multiplicities. JSON or instrumentation changes require new measured
checkpoints before making performance claims.

A later library benchmark should add separately named admission-only, solve-only
on the same immutable admitted artifact, and full composition measurements. Its
typed recorder/observer and semantic checks should be the same public APIs an
embedding application uses. It must not replace or relabel the ordinary CLI data.
No speedup or resource improvement is claimed by this architectural audit.
