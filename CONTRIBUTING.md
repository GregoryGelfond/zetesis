# Contributing to zetesis

Start with the [guided tour](docs/book/architecture/tour.md), then the
[library map](docs/book/rust/libraries.md). The [documentation build guide](docs/book/building.md)
explains how to build the manual and exercise its Rust examples.

zetesis implements exact answer-set solving through the reduct, with composable
parallel and device execution. It uses themelios for source representation and
analysis. Every supported profile follows the same correctness and authored-code
standards; unsupported features require explicit boundaries. The MIT license
names Gregory Gelfond.

## Design from the logic programmer's questions

Public semantic operations speak about programs, atoms, interpretations, answer
sets, consequences, objectives and proved conclusions. Read themelios-program and
themelios-analysis as examples: operations describe logical objects and their
properties; their implementation strategy is a lower layer. An unchecked
interpretation is not an answer set. A verified incumbent is not a proved optimum.
A stopped search is not an inconsistency result.

Reuse themelios's program, symbol, provenance and analysis vocabulary at the
source boundary. Native dense IDs, packed relations and formula DAGs are justified
execution representations with explicit validated conversions. Do not flatten
logical values into strings for internal exchange or add another ASP parser.
Keep the reviewed Git dependency pin. A dependency update must establish its
source, API and semantic compatibility explicitly.

Library-first means a consumer can use the capability without clap options,
process invocation, global I/O or parsing rendered prose. Typed configurations,
models, faults, limits and outcomes belong to the capability. Human, JSON and
consumer-specific views derive from those values. A CLI maps arguments, loads
sources, selects views and handles process exit; reusable orchestration belongs
below it. The [session boundary](docs/book/rust/sessions.md) documents the current reusable
entry points and their remaining integration limits.

Use modules within an existing capability by default. Add a crate when it creates
a useful independent dependency boundary, not merely because a diagram has
another box. Reuse one implementation across source and already-admitted input
doors. State ownership, failure semantics and costs on public operations. Keep
pure transformations separate from injected execution effects and resource
accounting; small named operations should make dependencies and parallel work
visible without excessive abstraction.

## Make represented knowledge inspectable

Name the audience at each boundary: ASP authors need the program's semantics and
admission limits; Rust consumers need ownership, effects, costs and typed errors;
maintainers need the invariants connecting the representation to the reduct.
Document departures from their expected semantics where they encounter them.

A distinction in a type must have a producer and a consumer. A consumer forced
to infer missing state from a string, sentinel or incidental control flow is
evidence that the representation needs refinement. Distinguish an arithmetic
consistency check from provenance, a candidate from an answer set, and recorded
evidence from a guarantee that the implementation establishes that evidence.

A name locates a concept in the domain's taxonomy; its full contract belongs in
documentation. Naming carries truth, including whether an operation computes a
value or writes to an external sink. Difficulty naming a function is evidence to
revisit its concepts and decomposition. Give meaningful steps and constants names
when the name adds knowledge; avoid aliases for literals that already express
their meaning. Represent absence explicitly rather than borrowing a valid value
as an undocumented sentinel.

One concept, one name. A word that denotes several things under-represents each
of them: give each thing its own name and use it in its context, and let a
shared mechanism keep its one name beneath them. Two words for one thing are the
same defect the other way: choose the name the thing's own module defines and
use it in the fields, the text, the serialized keys and the manual alike. Two
words that name two different things are not a conflict, even when the things
are related; state the relation once, where the second is defined. A generic
word qualified by its module, such as one measurement's `Case`, names the same
concept in each context and stays.

Use the logic programmer's word at a semantic boundary and an execution word for
an execution primitive, and let neither stand in for the other. A fluent reader
is surprised most by an unmarked departure from the field's term, so a departure
is documented where it happens. A name of the formalism carries a plain gloss
where a reader first meets it, "never: no seed of the region satisfies it", and
the name alone afterwards; the manual's [Vocabulary](docs/book/vocabulary.md)
page is the registry of the chosen names. A consumer-facing name, a serialized
key or a table column, changes only with its older spelling recorded beside the
retained records that use it, so that a record written before the change stays
readable. The [cancellation API migration](docs/book/rust/outcomes.md#cancellation-and-deadlines)
distinguishes the shared token from broader interruption variants and retained
record vocabulary. Executables follow the same rule: live documentation runs and
lists only binaries the workspace builds, which a portable-gate regression
checks, while a page recording a dated measurement carries the marker
`<!-- A dated record: its commands keep the spellings of the binaries it records. -->`
on a line of its own below its title, so that its recipes keep the spellings of
the binaries they record.

Design an algorithm with its correctness argument: preconditions, postconditions,
maintained invariants and a decreasing measure or finite bound for termination.
Name the concepts carrying that argument. Use pure transformations where they
make the knowledge explicit; keep state and execution effects at named boundaries.
An explicit cursor over foreign input needs its own frame invariant and resource
bound. Iterator syntax or structural recursion alone does not establish safety.

A new representation or API must explain the algorithm it serves, its ownership
and cost model, the resulting complexity, and why a simpler representation would
lose necessary information or increase that cost. State worst-case work and
retained space separately. Measure the expensive operation before claiming that
an optimization matters to ordinary solving.

Keep caller contracts and maintainer invariants self-contained. The manual explains current contracts and durable design arguments. Keep
development diaries and transient qualification records outside the source tree.
Retain curated fixtures, provenance and reproducible checks for public claims.
Documentation must be intelligible without access to private working records.

Keep the manual's runnable examples in shared checked source files. Link semantic
definitions and algorithms to named implementation APIs or modules and the
corresponding Lean declarations. During each change, follow affected references
and check both their targets and the claims they support. A working link alone
does not establish correspondence. Prefer stable include anchors to line-number
excerpts, and maintain the book with the implementation it describes.

The [language coverage checklist](docs/book/reference/language-coverage.md) uses
stable identifiers for known language boundaries. Keep those boundaries and
their linked evidence consistent with the admitted-language reference. Partial
support does not establish a complete contract, and unclassified scopes remain
explicit. Checklist counts do not measure a percentage of language parity.

## Carry the semantic contract into Lean and Rust

Use the same mathematical distinctions in the specification, public APIs and
Lean: original theory, interpretation, satisfaction, frozen reduct, stability,
coverage and objective ordering. Explain any representation correspondence.
State an optimization's applicability and preservation law before its physical
schedule. A class certificate may justify a specialized check; candidate
restrictions never replace the original theory whose stable models are sought.

New solving algorithms must compose reduct operations from reusable joins,
maps, masks, reductions and fixed-point computations. Do not introduce DPLL/CDNL
or migrate candidate generation or reduct checking to a SAT/CDNL architecture.
The existing optional clauses route is documented as an existing implementation;
its presence is not a design precedent for new work. Semantic planning precedes
physical scheduling: a checked class certificate has the same original-theory
meaning on CPU and GPU. State the actual host/device work and completion
requirements separately from the requested backend.

Separate three obligations: semantic laws, executable representation/refinement,
and measured backend behavior. Existing Lean laws do not certify Rust, shaders,
parsing or hardware. Do not use proof counts or passing regression tests as a
substitute for an unproved correspondence. New proofs use the pinned Lean
package, its audited axioms and source-hash record; no proof holes or hidden
assumptions. Follow the [structured proof convention](proofs/STYLE.md) and
[proof boundary](proofs/README.md).

For a solver release, restrict proof development to the fundamentals enabling
formal verification of zetesis: semantic laws, representation correspondence,
grounding/search coverage, reduct checking and honest bounded outcomes. Choose reusable
ASP and stable-model definitions, explicit module boundaries and careful source
attribution so these results can naturally become part of a broader Lean theory
library. That larger collection is a design horizon, not a requirement to
formalize unrelated literature or extract a separate package during solver work.

Keep model identity distinct from its displayed projection, publication distinct
from verification, and resource exhaustion distinct from exhaustive coverage.
Refuse unsupported constructs and exceeded limits explicitly. Interchange may
lose declared source provenance; it may not silently change answer sets, costs,
output or admitted control semantics.

## Verification and review

Follow the workspace's rustfmt, pedantic Clippy, documentation and authored
unsafe-code gates. The one Clippy threshold the workspace sets is in
`clippy.toml`, with its reason beside it. Resolve diagnostics in the
implementation; a passing command does not justify hiding an unused operation
or weakening a check. Do not suppress
`dead_code`, its `unused` parent group or `warnings`, whether with `allow` or
`expect`. Do not manufacture uses or widen visibility to evade these checks.
Shared test helpers should expose cohesive operations and be compiled only by
the consumers that need them. Helpers that several crates' tests share live in
internal support crates, each over one dependency closure, so a test build
compiles only what its tests use and no crate's unit tests link a second copy of
that crate. `zetesis-test-support`, over `zetesis-core`, holds the sinks that
fail on purpose. A helper that only one crate's tests use stays in that crate's
`tests/integration/support`. The support crates are not published, and every
coverage report skips their sources.

The portable gate's `authored_lints` regression checks literal Rust attributes
throughout the maintained source roots, including inactive `cfg_attr` branches
and literal macro templates. Quoted examples are not attributes. This complements
the compiler and review: it does not expand procedural macros or audit Cargo
configuration. Keep `dead_code = "deny"`; `forbid` conflicts with attributes
generated by clap derives and Rust's example test harness.

The oracle gate runs the ignored clingo comparisons its campaigns in
`scripts/check.sh` select by hand: test targets and, within a crate's
`integration` target, test-name filters naming modules. The portable gate's
`oracle_selection` regression reads those campaigns and fails when a test whose
ignore reason names clingo is selected by none, or a campaign's selection holds
none. Select a new comparison's module there when the comparison is written.

A foreign API can require a signature or field name that conflicts with a style
lint. Such an exception must use a narrow `expect` on the required declaration
and explain the actual interface constraint. It remains an exception, subject to
review; this is not permission to suppress implementation defects. Avoid
unbounded recursion on foreign input, unchecked arithmetic and hidden allocation
or fallback behavior. Make malformed input, cancellation, partial results and
writer/device failures intelligible typed outcomes.

Use `scripts/check.sh portable` for portable Rust tests, lint, strict docs
and benchmark correctness, including the three maintained standalone Rust packages.
Their checks remain separate from the workspace coverage population. Run
`scripts/check.sh oracle` for the relevant external
clingo comparisons, `scripts/check.sh coverage` for both independent 91% line
coverage floors, and `scripts/check.sh proofs` when proof sources or records change.
Local `scripts/check.sh coverage --metal` adds 58 exact physical tests within
workspace coverage: static constructor and complete closure/reference checks,
native aggregate reduction, lazy transport
and source closure, typed relation masks, tight and formula
oracles, shared-context composition and failure handling, and ordinary
lazy/formula CLI paths, complete-world-view collection and caller-owned session
resources with exact executor context and compiled-profile identity, and combined
head, objective and output contracts over complete answer-set families. The
session group also checks complete terminal-definition reconstruction over
device-verified base answers, automatic tight membership on the device, general
device checking for non-tight theories, and tight work refusal before dispatch.
These are the current required tests, not a claim that a newer source has been
physically qualified; recorded coverage remains bound to its stated source.
The formula CLI group also checks completed-support table joins with actual table
probes and GPU candidates against complete CPU/Metal answer families.
Every target group must report its
expected named passing tests. `scripts/check.sh hardware` qualifies the host's
own device backend without instrumentation, Metal on macOS and Vulkan
elsewhere, or the one named by `--metal` or `--vulkan`: the same fourteen groups
of 58 exact tests, each backend's reviewed selection, checking complete CPU/device
answer families and explicit failure boundaries; a change to a device route is qualified on
every backend the hosts at hand expose, and the record says which. The
portable report is retained separately; the CPU-only profile independently
instruments both `zetesis-solve` and `zetesis-cli`. `target/coverage/toolchain.json` records the finite selection, with
per-group logs and status files under `target/coverage/workspace`; the
hardware gate keeps its logs and status files under `target/hardware`. Neither
floor nor filename filters change. Unlisted GPU paths still require their own
physical qualification.
After both reports are written, the gate checks both floors even if the first
fails. `target/coverage/floors.tsv` retains each profile's exit status;
`target/coverage/status.txt` remains incomplete unless both floors pass.
Run `scripts/check.sh book` for the checked manual. It requires mdBook 0.5.4;
Lean uses the toolchain pinned under `proofs`. External comparisons require
clingo 5.8.2 on `PATH` and its absolute executable path in `CLINGO`, as described
in the tool setup guide. Tests should exercise semantics and failure boundaries,
including property and adversarial cases; do not mirror the implementation or
weaken gates to accommodate a feature.

Write the proposition before changing its implementation, then demonstrate that
the assertions detect its violation. Tests claiming multiple execution routes
must check which route actually ran. A test claiming retained results must inspect
their identity and evidence. Coverage locates unexercised code; it does not prove
assertion strength. Selected negative controls, systematic mutation campaigns,
input fuzzing and mathematical proofs discharge different obligations. Report
the instrument and scope actually used.

Before adding a test for uncovered code, establish that the code still serves
an admitted behavior or necessary failure boundary. Remove obsolete states and
unreachable responsibilities when their absence is justified; retain necessary
defensive behavior even when exercising it requires a separate qualification.

A test name states one proposition. Split independent claims joined by a
conjunction into separate tests, with meaningful shared setup where useful.
Rationale belongs in a comment, not in the identifier. Names over fifty characters
trigger a reading for hidden conjunctions or appended rationale; fifty is not a
length ceiling. A longer name is appropriate when it states one coherent claim.
Multiple assertions may jointly establish one invariant, and multiple inputs may
exercise that same proposition; neither test count nor identifier length measures
assertion strength.

Clingo is an external qualification oracle, never a production solver dependency.
Its observable results corroborate the intended answer-set semantics; its
internal representations and algorithms are not an implementation specification.
Investigate a disagreement against the declared semantics rather than copying
internal behavior or assuming either implementation is correct.
Preserve original corpus inputs. Retain complete models, optimal ties, objective
priorities and completion evidence in comparisons. Match candidates and work
where a comparison claims to do so; record toolchains, binary/source identities,
resource limits and actual backend execution. CPU coverage does not qualify a
physical GPU. Performance claims need reproducible end-to-end evidence, including
grounding, solving, output, transfer and exact residual work where applicable.

The [manual](docs/book/index.md) has three parts: solver architecture, the library
programmer's manual and the Lean proof library. Keep all three current with the
implementation. Lean sources, theorem statements and proof checks are
programmatic deliverables, not deferred documentation. Update claims and
limitations together. Comprehensibility to an ASP/reduct practitioner is a review
criterion alongside correctness and performance.

Give every baseline, candidate and mutation checkout its own Cargo target
directory. A primary checkout may share its target among its own package checks;
an alternate source tree may not reuse it. Preserve source and executable
identities with experimental evidence. If a target has been shared across those
trees, treat its checks as provisional and qualify again from a fresh target.
Matching Cargo artifact filenames do not attest which source tree produced them.

## Build storage

Give each build directory an owner and a retirement point. Reuse an active
checkout's target for compatible checks; keep distinct source trees and
instrumentation populations separate. Do not create another full build tree
without a concrete isolation or comparison need. Check available space before
large builds, accounting for temporary compiler output and retained baselines.

At closeout, retain the source revision, tool and command identities, reports,
required profiles, and the specific binaries or other inputs needed for pending
qualification, reproducible evidence or the next comparison. Store those durable
artifacts outside disposable build directories and verify the retained copies.
Then remove completed build caches, including incremental state and superseded
targets. Record any build tree that must remain and the condition for retiring it;
a historical path in a log alone does not make an entire target tree permanent.
Keep one comparison baseline unless the next experiment needs more.

Preserve unique source files, unpublished work and active qualification inputs.
Coordinate cleanup when another process or contributor owns the directory.
Failed checks retain their reports and failure evidence; retaining a failure
does not require keeping every rebuildable dependency artifact indefinitely.

## Repository presentation

Keep the GitHub description, topics, README badges and release metadata aligned
with the maintained language and execution references. Describe implemented
capabilities separately from planned work. Lean semantic laws do not justify a
formally verified implementation badge; physical Metal tests do not qualify
other GPUs. A CI badge must identify an active workflow, and a coverage claim
must identify its measured source revision, profile and backend scope. Do not
substitute local results for hosted CI status.

Language statistics describe tracked code. Use Linguist attributes only for
documented provenance: preserved third-party source is vendored, and generated
snapshots are generated. Keep authored tooling and runtime shaders visible even
when their languages differ from Rust and Lean. Removing a language from the
project requires replacing or retiring its maintained code, not concealing it
from the statistics. Preserve byte-exact corpus attributes and source hashes.

Release tags and notes must correspond to the version actually released and its
qualified scope. Historical archive tags are not releases. Link badges and the
repository homepage only to resources that exist and are accessible to their
intended audience; update version, license and toolchain claims together with
their source declarations.
