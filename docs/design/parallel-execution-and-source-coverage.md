# Parallel execution and source coverage

Status: design sequence following the `7690475` release and its
[instrumented M4 Pro matrix](../verification/dependencies-measurement-tranche-20260908/physical/README.md).
The isolated [certified Metal experiment](metal-tight-support.md) is the first
implementation step. The remaining steps below are not yet implemented by this
plan. They supersede earlier tranche priorities without changing historical
qualification records.

The original program and its frozen Ferraris reduct remain the acceptance
criterion. A certificate can discharge a reduct obligation for a checked class;
it cannot change the program being solved. Source analysis, grounding strategy,
membership procedure and hardware placement are distinct decisions. themelios
supplies source objects and analysis through the unchanged dependency pin.

## Evidence determining the order

The corpus matrix has five timed observations for each of 94 inputs on eager
CPU, eager Metal and clingo. Both lazy profiles refuse all 94 formula inputs.
CPU uses certified tight-support checking on 53 cases; Metal always uses the
general countermodel procedure. Its higher median time on every input therefore
does not isolate the effect of hardware. Fresh Metal setup contributes about
8.6 ms, while larger task-allocation cases also require substantial CPU residual
completion. The full timing record retains phases, output costs and all cases.

SEND still spends about 17.8 ms in eager grounding. Queens variant 02 spends
roughly 81–84 ms in candidate generation. These identify different operations to
investigate; they do not justify one universal optimization. The earlier
controlled binary-scan change improves queens and task allocation while SEND
remains flat. Peak process memory remains unmeasured.

The [planning probes](../verification/execution-planning-20260908/README.md)
confirm additional internal source refusals on the sealed release. Weighted and
extremal heads, aggregate-dependent objectives and conditionals, inverse affine
arguments, conditional disjunction, projection and explicit program/external
directives remain real gaps. A bounded-choice control succeeds eagerly and
refuses lazy execution. These examples are not a whole-language acceptance rate
and do not classify those valid clingo inputs as modeling errors.

## 1. Match the certified procedure across executors

First qualify batched original-formula evaluation and producer support on an
existing complete `TightPlan`. Scalar, Rayon and Metal receive identical ordered
candidate occurrences, including nonmodels and residuals. Independent tiny
frozen-reduct checks and complete general reduct checks qualify the references.
Every residual still receives exact completion. Fresh and resident GPU storage,
setup, transfers, classification, CPU completion and failures retain separate
records. No ordinary dispatch policy changes merely because this primitive
compiles.

After physical qualification, integrate the checked procedure as a library-owned
strategy with CPU/Rayon/device executors. Prepare the semantic strategy before
constructing an unnecessary general GPU oracle. Preserve original-theory identity,
candidate accounting, objective ties and interruptions. CPU certificate decisions
must not be counted as GPU decisions. Explicit hardware requests remain visible;
automatic cost selection operates only among eligible implementations.

Measure both ordinary fresh-process solves and amortized batches using the same
strategy. Retain a general-reduct control. A useful device range, rather than a
universal GPU win, is a valid outcome. If no useful range is demonstrated, retain
the evidence and qualified primitive while leaving ordinary defaults unchanged.

## 2. Extend lazy execution to a bounded formula fragment

Make the first slice finite normal/choice rules with integer cardinality bounds,
using `1 {a;b} 1.` as an admission control and parameterized relational choices as
scaling fixtures. Establish its source-instance and candidate-carrier contracts
before expanding to dependent aggregates or SEND's arithmetic. Reuse the source
program, binding and dependency plans; do not implement another parser or relabel
complete formula materialization as lazy execution.

The [bounded-choice design](lazy-bounded-choices.md) separates exact base reduct
closure from completed validation of the original cardinality groups. It reuses
the lazy normal/choice engine and keeps a complete gate carrier. Active groups
with zero eligible rows and duplicate heads require explicit treatment. The first
shared API change exposes checked outer bindings from the existing join engine;
no complete table of all possible group witnesses is required for final checking.

The engine must retain obligations for instances and atoms not yet discovered.
A candidate can be accepted only after every relevant original-rule and frozen
reduct obligation is discharged or covered by an exact representation. Growth
invalidates receipts for an older subject unless a proved extension law applies.
In particular, tightness of the currently observed fragment does not certify the
complete source. Source joins may remain on the host in the first slice, with
genuine per-candidate consequence/checking work on Metal.

Deliver a bounded, exhaustive-or-error source protocol with eager reference,
Rayon and Metal execution, and interruption tests at discovery, transfer and
commit. A resumable cursor is a later extension with explicit source, candidate
and budget ownership; the existing callback scanner does not already provide it. Record
requested/effective grounder, catalog growth, generated instances, charged bytes
and actual device work. Compare complete answer sets and optimum ties where the
slice admits objectives; keep unsupported combinations explicit. A bounded first
slice is progress toward the 94-case lazy gap, not closure of that gap.

## 3. Close source gaps through shared aggregate machinery

Changes to binding and aggregate IR must establish shared semantic foundations
before dependent consumers use them:

1. Admit completed aggregate values in the remaining conditional consumer scopes,
   preserving local versus outer bindings and the original implication.
2. Extend objective-relevant consumers with an explicit activation/totality
   contract. Proposed aggregate values do not establish an objective's value or
   existence. Preserve global objective keys, priorities and all optimal ties.
3. Add weighted head aggregates through the existing checked tuple/atom
   correspondence, beginning with a precisely admitted `#sum`/`#sum+` fragment.
   Then add extrema heads with their complete-value and empty-set semantics.
   Broader aliases and recursive conditions need their own preservation evidence;
   do not erase the current restriction without replacing its argument.

Each slice needs original clingo examples, independent arbitrary original/frozen
interpretation checks, recursive and empty controls, duplicate tuples, source
order permutations and exact resource stops. Reuse themelios's represented
constructs. Continue to distinguish internal limitations from front-end numeric
guards and explicit exclusions. `#heuristic`, `#edge`, theory atoms and Python/Lua
scripting remain excluded. Projection, program parts, externals, other remaining
binding forms and Rust `@` functions retain their separate obligations; this
sequence does not declare them implemented or silently remove them from scope.

## 4. Reduce measured host work and expose its costs

Use the existing eager grounding attribution and candidate-search measurements
to select general operations for controlled changes. Candidate hypotheses include
indexed arithmetic/binding operations and avoiding repeated relation or formula
lookup, with `zetesis-domain` as the independent domain-analysis boundary.
Affine inversion can improve admission and binding, but must have explicit
checked-integer preimages, divisibility and finite-domain conditions. SEND's
identity must never be an optimization trigger.

Keep baseline/candidate builds matched, retain all observations and compare
ordered subjects, complete answer contracts, typed failures and work limits.
Measure allocation or peak RSS before claiming memory gains. Report-size
compression, logical retained bytes, transfer bytes and resident memory are
different quantities. Include dense controls that expose index or mask overhead.

## Integration and completion

Shared semantic representations must be established before dependent consumers
use them. Each GPU, source-language and host/lazy change needs its own public API,
documentation, proof and regression qualification. Unfinished work does not
borrow the qualification of a completed change.

Every slice carries the same library-first vocabulary, explicit failure and
resource contracts, pure transformations where appropriate, and test propositions
that match their assertions. Correct misleading diagnostic terminology when a
changed boundary exposes it; avoid unrelated cosmetic churn. New semantic laws
must state their applicability and remaining Rust/WGSL correspondence. No proof
count establishes that the concrete solver is formally verified.

The checkpoint requires local macOS tests, strict rustfmt/pedantic Clippy/rustdoc,
relevant clingo parity, independent unchanged 91% workspace and CPU-CLI coverage
floors, current Lean records, sealed release artifacts and physical Metal
qualification for changed device paths. Publish performance and incomplete/refused cells with their actual
scope before changing defaults or making release claims.
