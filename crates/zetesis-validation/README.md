# Validation tooling

This package has three commands and reusable corpus, process-capture and reported-answer libraries. `zetesis-corpus`
verifies the selected clingo fixture or compares its complete solver results.
`zetesis-validate` runs the separate 94-case kr-domains solver campaign described
below. `zetesis-perf` characterizes CPU suites and explicit execution matrices using sealed inputs,
complete answer families and a fixed paired schedule. Its default remains the
three established comparison cases.
None of these commands is invoked by the production solver.

## Ordinary CPU performance characterization

```sh
zetesis-perf examples/kr-domains --zetesis /absolute/path/to/zetesis \
  --clingo /absolute/path/to/clingo --report target/new-cpu-comparison.json
```

`performance::run(&Request)` composes the clean-corpus loader, private source
copies, bounded process capture, display contracts and sealed input identities.
It retains every launched sample, including failed attempts. The default schedule
has three warmups and 21 alternating timed pairs per case, following a complete
qualification pair. Separate `--stats` invocations observe grounding and solving
without adding their instrumentation to the ordinary timing population.
Publication is a separate bounded, no-clobber operation.

The [protocol](../../docs/verification/ordinary-cpu-refresh-20260907/PROTOCOL.md)
states the exact execution order, worker settings, process/output limits and
comparison scope. Peak RSS remains unavailable. This legacy protocol does not execute the
instrumented matrix described below.

`--suite queens` selects all six real N-Queens source encodings in variant order,
at their existing eight-queen setting. Variant 01 uses literal bounds; variants
02–06 declare `#const n=8`. The runner rewrites neither source nor constants.
The default population becomes 306 solves, including 12 initial qualification
observations, 252 timed observations and six separate diagnostics, plus the same
three executable metadata calls. Source, process, capture and publication ceilings
remain unchanged; a larger suite may stop under those bounds.

Library clients use `Schedule::for_suite(Suite::Queens, warmups, repetitions)`.
`Case::ALL` lists all eight supported inputs; `Suite::cases()` states a selection
and its order. `Case::Queens02` preserves the historical serialized identifier
`"queens"`. Baseline schedules retain their original JSON representation and exact
slot order; an explicit queens schedule additionally records `"suite":"queens"`.

## Instrumented grounding/backend matrix

```sh
zetesis-perf examples/kr-domains --suite corpus \
  --zetesis /absolute/path/to/zetesis --clingo /absolute/path/to/clingo \
  --workers 4 --completion-workers 4 --clingo-workers 1 \
  --repetitions 5 --warmups 1 --campaign-seconds 1800 \
  --sample-bytes 33554432 --native-report-bytes 33554432 \
  --capture-bytes 536870912 --report-bytes 2147483648 \
  --report target/new-instrumented-matrix.json
```

`performance::matrix::run(&Request)` accepts a typed `Plan`, independent of the
command. `--suite corpus` requests all 94 clean cases and the four explicit
CPU/eager, CPU/lazy, Metal/eager and Metal/lazy profiles. Repeat `--profile
cpu-eager`, `cpu-lazy`, `metal-eager` or `metal-lazy` to choose a bounded subset;
these profiles also work with `--suite baseline` or `--suite queens`. Matrix
native worker defaults are four for both closure and formula completion; clingo
uses one. This unequal allocation is recorded. Explicit worker/batch controls
require a matrix request and never silently alter the legacy one-worker protocol.

Every native invocation uses `--json --stats --models 0`. Native time therefore
includes instrumentation and full typed JSON publication, while clingo time
includes its JSON publication. These are **instrumented end-to-end observations**,
not replacements for the historical uninstrumented CPU timings or fulfillment
of every condition of the [full protocol](../../docs/design/corpus-performance.md).
Native JSON retains full atoms while clingo JSON exposes selected displays, so
their measured output volumes are asymmetric. CPU/Metal profiles of the same
native source use the same output contract. Clingo provides parity and a recorded
wall-time reference here; this matrix does not isolate algorithm performance
between solvers.
The example presets 3,290 positions: 94 cases, five producers, one census, one
warmup and five timed rounds. Each producer occupies each timed position once.
The matrix defaults to 20 timed rounds and three warmups. Reference-first census
positions precede later rounds; case and configuration positions then rotate.
With four native profiles, each configuration occupies every timed position four
times. Source constants and output directives are never rewritten.

A cell's first refusal, timeout, capture stop, mismatch or invalid observation
retains the raw invocation and disables only that cell's later launches. Every
scheduled position remains represented; skipped positions refer to the earlier
failed sample where applicable. Other cells continue until the shared campaign
or capture bound, unresolved-child cleanup or reference failure prevents them.
No replacement samples are taken. `accounted` means every schedule position is
represented; `passed` requires every requested observation to pass. Exit 1 and
`non-pass cells retained` are expected when unsupported lazy combinations appear;
setup/publication errors exit 2. Results are held until bounded no-clobber
publication; the runner does not provide crash recovery or campaign resumption.

Native typed full atoms, shown terms, objective priorities and raw statistics
remain in captured JSON. The checked cross-solver contract is **complete selected
displays, symbol/model multiplicities, final costs and optimum ties**. Hidden
clingo interpretations are unavailable through its ordinary JSON output; this is
not full-model parity. `NativeAnswers::reported_displays` is a bounded library
view that retains collisions between shown atoms/terms and equal displays from
different hidden models.

Matrix capture streams serialize as `{"encoding":"utf8","data":"..."}`
when the complete captured prefix is valid UTF-8, or
`{"encoding":"bytes","data":[...]}` otherwise. Both views preserve every
byte, including empty streams, NUL/control characters and malformed UTF-8.
This borrowed publication view avoids expanding ordinary JSON into integer
arrays; the legacy capture schema is unchanged. Serialization occurs after
measured invocations and remains subject to the publication ceiling.

Each complete native observation reconciles the existing authored route text
with typed JSON statistics and the strict phase/stage parsers. Requested and
actual grounding/backend remain distinct; conflicting or missing route evidence
invalidates the observation. Lazy grounding stays interleaved with solving.
Lazy device dispatch/transfer counts and formula propagation/CPU residual counts
are retained where reported. The static closure driver currently exposes its
adapter but does not accumulate device work/transfer totals; those quantities
remain explicitly unavailable. A measured zero is retained as zero and does not
establish acceleration. Generic GPU failures are not relabeled as unavailable
adapters. Instrumented producer telemetry is evidence from a trusted executable,
not independent hardware attestation or a GPU kernel timer.

`--timeout-seconds`, `--campaign-seconds`, `--sample-bytes`, `--capture-bytes` and
`--report-bytes` independently bound capture and publication. Matrix-only
`--native-report-bytes` sets native JSON decoding's input ceiling independently
of process capture; its default remains 8 MiB. Library requests
also own typed answer/value and spelling limits. Large full-model JSON families
may reach those limits; raw failures remain evidence and no ceiling is silently
increased. The report records source/include and binary hashes before/after,
private-copy identities, exact arguments, every capture and all configured
normalization limits. Peak RSS remains unavailable: the shared safe process
boundary does not retain per-child rusage, and logical host/GPU counters cannot
substitute for it. Measurement requires a qualified binary, a quiet host window
and a context exposing the requested Metal device. No physical or timing result
is implied by portable tests of the runner.

## Reusable process and reported-answer boundaries

`zetesis_validation::process::invoke(Invocation, Limits)` accepts an explicitly
resolved absolute executable, arguments and working directory. It returns raw
stdout/stderr prefixes, a typed stop reason, direct-child exit evidence, elapsed
time, and separate capture/cleanup faults. `Stop::Completed` means the direct
child was reaped and both streams reached EOF; it does not mean a solver completed
enumeration or returned correct answers. It establishes no descendant-termination
property: a same-group descendant that closed both pipes may still survive.
Campaigns must establish trusted solver behavior and coordinate owned jobs
separately; process completion is not a system-quiescence certificate.
Nonzero exit codes remain ordinary
process evidence for the caller's exit policy. Strict UTF-8 accessors never
replace invalid bytes.

The strong backend is implemented for Linux and macOS using safe `rustix` APIs.
It starts a fresh process group, alternates bounded nonblocking reads, and waits
only when neither stream progresses. The direct child remains waitable until
after any required group signal, so the runner does not signal a reused numeric
group ID after reaping. Normal completion only reaps; deadline, byte-limit and
capture failures attempt group termination. No reader thread or blocking wait
can outlive the call. Descendants that leave the process group are outside those
termination attempts. Normal completion does not attempt group termination,
including for same-group descendants with closed pipes. Callers must not
independently reap the child.

The shared raw byte ceiling is inclusive and checked before retention. Vectors
use fallible amortized growth; capacity and OS pipe storage are separate from
retained bytes. A main deadline and a separate cleanup interval bound authored
polling, not OS-call or allocator latency. If cleanup cannot establish reaping,
the returned `PendingChild` retains ownership. A caller must retry for an explicit
interval or report explicit abandonment; `Drop` performs no wait or background
work. The validator stops launching cases, retries once, and records any
unreaped child ID at its failure-report boundary.

The older Rust validator retains its previous backend on other platforms; that
backend has weaker direct-child and pipe-thread cleanup guarantees. It has not
been newly qualified by this change. The stronger reusable operation returns
`UnsupportedPlatform` there. This platform distinction does not silently remove
the older command's path. The selected Rust campaign requires the stronger
Linux/macOS process backend.

`zetesis_validation::answers::clingo_json` and `native_text` return immutable
`ReportedAnswers` under explicit input, witness, symbol-occurrence and objective
dimension ceilings. Producer grammars are separate from corpus annotations and
from process execution. The native text entry point is explicitly a legacy
presentation adapter. The result reconciles reported status, costs, counts and
display multiplicities; it does not reconstruct hidden full interpretations or
certify the producer's claims. `same_displays` compares exactly that evidence.
The library preserves repeated symbols inside a display and repeated equal
displays, including optN's exact single-incumbent removal. Malformed discarded
incumbents remain refusals. Input bytes bound initial JSON decoding; these limits
are not heap/RSS accounting.

`answers::native_json::parse` separately checks native schema-1 full-model
records. It returns core `Atom`/`Value` data and retains shown atom positions,
shown terms and priority/cost pairs as distinct views. It reconciles exhausted
coverage, publication/verification counts, exact optimum and all retained ties;
a failed envelope cannot qualify through retained optimality data. These are
checked producer claims, not an independent solver-correctness proof.

The raw typed decoder accepts the core's broader name domain.
`NativeAnswers::full_model_symbols` validates predicate, symbol and function names
with the pinned themelios identifier lexer before exposing an ASP comparison
view. Ambiguous names such as a nullary predicate named `p(a)` are refused rather
than confused with the atom `p(a)`. Structural spelling reuses the core's cached
renderer; a small outer/scalar view supplies predicate punctuation and the same
string escaping where core `Atom` and scalar `Value` have no `Display` implementation.
The view preflights exact output bytes. Identifier validation temporarily holds
two copies of one admitted name, separately from retained output and value data.

Native value construction uses the core's shape/depth/payload checks. Initial
JSON and temporary decoded node/text storage are bounded by the report bytes and
node ceilings; per-value limits are not a pre-allocation or RSS ceiling for the
whole decoder. JSON object decoding follows serde's last-key-wins behavior;
duplicate object keys are not currently refused. Integer costs remain exact
signed 64-bit values; consumers of serialized evidence need lossless integer
handling rather than JavaScript floating-point coercion.

The current 94-case command uses these shared implementations on Linux/macOS and
keeps its historical report fields through adapters. Invalid UTF-8 cannot become
completed text: a failed record labels its lossy legacy view and retains the
original bytes. The selected 24-case campaign composes the same process boundary
with native typed JSON. Its predecessor Python campaign and decoder are retired;
retained C++ provenance still supports the explicit Rust import operation and
its independent integrity tests.

## Selected solver campaign

`selected::run(&Request)` returns immutable evidence for all 24 original cases.
The library accepts explicit executable paths, execution requests and logical
resource ceilings; it performs no PATH lookup or global output. Its report exposes
typed case decisions, invocation arguments, raw output, failures and primary-file
identities. `Report::publish` is a separate effect that refuses existing paths,
input aliases and reports exceeding their byte ceiling. Its parent directory
must remain under the caller's exclusive control; this is not a filesystem lease.

```sh
zetesis-corpus compare validation/upstream/clingo-5.8.2/curated \
  --clingo /path/to/clingo --zetesis /path/to/zetesis \
  --report target/upstream-parity.json
```

`passed()` requires every case to complete and agree with the pinned full-model
multiset, every input seal to remain unchanged, and all child/input cleanup to
succeed. The selected sources have no output projection or objectives, which
justifies treating clingo's complete shown atoms as full identities. That
implication does not extend to arbitrary programs. The native view always comes
from typed full atoms. Original helper prefix-selection contracts are checked
separately. A refusal, timeout, stopped enumeration or malformed report never
counts as a pass.

The CLI publishes both passing and failed campaign evidence, then exits 0 or 1
respectively; setup/publication errors exit 2. Reports record authored limits,
requested execution and exact stdout/stderr bytes as JSON byte arrays. Seals
cover primary executables, manifest, license and sources, not dynamic libraries,
the inherited environment or modifications restored between checks. Captured
timings are diagnostic and are not a controlled performance experiment. A
requested Metal backend alone does not qualify device execution.

## Selected clingo corpus

From the repository root, verify the checked-in data with the pinned Rust
toolchain:

```sh
cargo run --locked -p zetesis-validation --bin zetesis-corpus -- \
  verify validation/upstream/clingo-5.8.2/curated
```

Normal verification needs only `curated/manifest.json`, `curated/LICENSE.md`,
and the 24 `.lp` files under `curated/programs/`. It does not read C++ files,
invoke the import decoder, start clingo, or require another estate checkout.
The sources are ordinary ASP files with their exact original bytes; they can
also be passed directly to a solver:

```sh
clingo validation/upstream/clingo-5.8.2/curated/programs/lparse/projectionBug/01.lp 0
```

To install only the integrity command into Cargo's binary directory:

```sh
cargo install --path crates/zetesis-validation --bin zetesis-corpus --locked
zetesis-corpus verify /path/to/zetesis/validation/upstream/clingo-5.8.2/curated
```

An explicit import reproduces the curated files from the retained legacy
catalog and pinned originals. Use a new destination whose parent exists:

```sh
corpus_import_dir=$(mktemp -d)
cargo run --locked -p zetesis-validation --bin zetesis-corpus -- \
  import validation/upstream/clingo-5.8.2 "$corpus_import_dir/curated"
cargo run --locked -p zetesis-validation --bin zetesis-corpus -- \
  verify "$corpus_import_dir/curated"
```

Import checks all three original file hashes, exact assertion byte/line spans,
section ordinals, decoded adjacent C++ strings, source hashes and helper
contracts before creating output. The decoder accepts only the literal subset
needed by these pinned assertions; it is not a general C++ parser. Import never
replaces an existing path or writes inside the original-source directory.
Publication requires exclusive ownership of the destination parent and is not
crash-atomic. A write or final verification failure attempts to remove the new
directory; a cleanup failure retains both causes explicitly.

The manifest pins 24 assertion identities and 73 complete model occurrences.
It keeps complete models separate from the original helper's prefix-selected
displays, preserving repeated and empty displays. The exact original helper
expectation also retains its diagnostic text. This integrity check establishes
agreement among the recorded contracts, not a fresh enumeration or native
admission verdict. Native admission labels stay in the existing caller policy
and cannot change this curated target.

Each case resolves to an immutable upstream URL, original file hash, assertion
span and exact spelling through the manifest. `LICENSE.md` is the upstream MIT
license; `origins[].copyright_notice` retains each original file's complete
notice verbatim. The `.lp` files contain no added comments or terminal newlines.
The manifest itself has a SHA-256 pinned in the Rust library; deliberate target
changes require updating both identities after review.

Rust clients use `zetesis_validation::curated::open(path, Limits)` or the
explicit `import_legacy` function. The returned `Corpus` owns verified sources;
`Case`, `Provenance`, `Origin` and `Contract` provide borrowed views without
public unchecked constructors. Source strings remain stable if the filesystem
later changes. Returned paths describe what was read and do not reserve those
files for a later solver invocation. A consumer needing exact verified input
should use `Case::source()`.

`Limits` bounds serialized manifests/catalogs, individual and cumulative source
bytes, individual original/license bytes, cases, full-model occurrences and
atom occurrences. Limits are inclusive; zero means zero. JSON containers are
allocated from the bounded serialized document before contract validation.
Import retains the three bounded original texts while checking assertions.
These are input and retained-source measures, not heap or RSS accounting.

Both commands provide machine-readable evidence. `zetesis-corpus verify` writes one JSON
record only after successful verification, with `semantic_solver_run: false`.
It exits 0 on integrity success and 2 on argument, integrity, filesystem or output
failure. Diagnostics go to stderr. A stdout write failure can leave a partial
record; consumers must check successful process completion before accepting it.

The Rust source/ordinary-solver regressions and selected campaign read this owned
curated corpus directly. The explicit import tests still read `cases.jsonl` and
the retained C++ originals; their independent provenance checks remain active.
See the [migration record](../../docs/verification/corpus-curation-20260907/README.md)
for the historical first-slice boundary. The selected Python comparator and
decoder are now retired; other Python qualification tools remain, so repository
cleanup is not complete.

## Full kr-domains regression gate

`zetesis-validate` is an independent validation executable. It is not linked to
or invoked by the production solver. The default target is the self-contained
`examples/kr-domains` collection: all 94 non-clingcon cases and their 14 schema
dependencies at revision `38f0660ded448ed268c5a68759ceb0e2840dd497`, with elenctic
annotation comments removed. No network access or estate checkout is needed.
The pinned manifest preserves typed contracts, original and cleaned hashes,
and exact annotation deletion provenance.

```sh
zetesis-corpus verify-examples examples/kr-domains
zetesis-corpus verify-examples examples/kr-domains --originals validation/corpus/kr-domains
```

The first command uses only the clean collection. The second independently
verifies its derivation from preserved originals. Both report integrity,
not fresh solver execution. Library clients compose `examples::load`,
`examples::verify_originals` and `Contract::check` directly; the last operation
requires the caller to establish successful process capture and producer completion.

```sh
zetesis-validate --repo /path/to/zetesis --report validation-report.json
zetesis-validate --repo /path/to/zetesis --reference-only --report reference-report.json
zetesis-validate --repo /path/to/zetesis --native-backend metal --native-oracle countermodel --native-batch-size 64 --report fresh.json
zetesis-validate --repo /path/to/zetesis --native-backend cpu --native-completion-workers 4 --native-max-completion-scratch-bytes 268435456 --report cpu-batched.json
```

The installed validator and solver run directly through PATH. Supplying
`--corpus` or `--manifest` explicitly selects the historical original-source
manifest mode, permitting reproduction of earlier campaigns. `--clingo` and `--zetesis` select executable
paths or names resolved through PATH. A relative executable path is resolved
relative to the caller's working directory, not the corpus. `--help` lists
resource limits. Omitting `--report` writes the JSON report to stdout; progress
goes to stderr.

The exact manifest SHA-256 is pinned in the validator; changing its whitespace,
paths, hashes, includes or contracts requires a deliberate trusted-target update.
Every referenced source is SHA-256 checked before any child is spawned.
The 94-case command then executes source paths; callers must keep the collection
unchanged during execution. It neither reserves those paths nor reseals them
afterward. The selected and performance libraries separately use private source
copies and before/after identity checks.
Relative paths must stay within the corpus; include hashes must match the
pinned schema entries. The report records the manifest hash, source hashes,
actual reference version, process exit codes, timing, captured diagnostics,
normalized models, costs, counts and per-case decisions.

The reference uses clingo JSON with complete enumeration and `optN`. Only
final-cost witnesses are retained, then exactly the first final-cost incumbent
discovery is removed. Its display must recur in the remaining optimal witnesses.
The comparison preserves the multiplicity of every displayed symbol list, including
empty displays, alongside the reference's raw `Models.Optimal` or `Models.Number`
count. Equal display sets and equal total counts alone are insufficient.
Raw witness counts must reconcile with the JSON summary. The supported clingo
5.8 optN format has exactly one final-cost incumbent replay in addition to its
declared optimal enumeration; incompatible output is refused explicitly.
The clean manifest translates historical elenctic expectations into typed
satisfiability, count, cost, witness and required-display-symbol contracts.
The historical mode retains its annotation contract adapter and refuses unknown
expectation tags. The solver receives ordinary ASP source only. Notes are prose.
Strings with spaces, commas and escaped quotes remain whole atom values.
Within one display, symbols are sorted and every occurrence is retained: an atom
and a shown term can print the same symbol twice. Hidden atom identities
cannot be reconstructed from `#show`; this corpus contract does not establish
them. The optimization-aware timing companion uses the same rule for repeated
symbols within each display.

Native runs default to `zetesis --backend cpu --oracle auto --models 0 --batch-size 64
--completion-workers 1 --max-completion-scratch-bytes 268435456` on each complete
source graph. The integrated release campaign passes all 94 cleaned cases,
preserving 1,931 selected model occurrences and the original contracts. The
previously qualified campaign on original sources remains a separate historical
record. These results cover the pinned non-clingcon corpus and
its original contracts; broader clingo language coverage is a separate target. Optimized
answers require per-model cost vectors and exhausted search coverage; comparison
preserves full optimal model counts, repeated displays and repeated symbols within
each display.
`--native-oracle closure|countermodel` permits explicit procedure comparison and
records the requested oracle and exact native arguments. It never narrows the
required 94-case target.

`--native-backend cpu|auto|gpu|metal|vulkan|dx12|gl|nvidia` passes that exact
hardware policy to the native solver. `--native-batch-size` requires a positive
integer; `--native-stats` captures the native statistics.
`--native-completion-workers` defaults to one and must be positive;
`--native-max-completion-scratch-bytes` defaults to 268435456 (256 MiB), accepts
zero, and is passed unchanged to the solver. The byte setting governs logical
completion batches, not the default CPU scalar cursor or process memory.
A zero allowance is a valid request that may produce a recorded native incomplete
result; the validator does not increase it or silently select another route.
The JSON report retains both completion settings, exact per-case child arguments,
and both the requested statistics flag and whether statistics were enabled.
Workers above one automatically enable statistics for reproducibility. CPU
campaigns retain their raw execution and phase records; physical qualification
remains a separate requirement for explicit physical countermodel campaigns.
An explicit physical backend combined with `--native-oracle countermodel`
selects physical formula qualification and automatically enables statistics.
The validator invokes the requested executable directly, with no alternate
solver, fallback or wrapper. It does not run device setup scripts.

Physical formula qualification requires actual hybrid-route adapter telemetry
for every completed case, matching the requested API or NVIDIA vendor policy.
Candidate, committed GPU decision, exact CPU residual, countermodel and retained
result counters must reconcile, and completed dispatches must respect the
reported batch and authored allocation limits. Missing, duplicated, malformed
or contradictory statistics produce `native_execution_unqualified`; matching
answers alone cannot pass that qualification.

The parser distinguishes two retained telemetry profiles. Historical scalar
records preserve their old GPU/candidate checks and record `completion: null`;
that absence never becomes evidence of a bounded scratch contract. Those records
cannot qualify a non-default worker or scratch request. Current records carry
`completion.profile: "bounded_completion_v1"` and must match the requested
completion workers and byte ceiling. Entered and completed checks, residual
completions, zero failures/pending/queued results, admitted concurrency, and the
logical scratch peak must reconcile with complete GPU/countermodel accounting.
Certificate-only batches may admit zero query workspaces. A completed outer-UNSAT
case must have zero scratch and dispatch work. Requested pool size, admitted
query concurrency, logical scratch and authored GPU bytes remain distinct.
Mixed profiles, duplicated records and overflowed completion counters fail
qualification. Original historical fixtures remain unmodified.

The report distinguishes `full_native_answer_parity_passed` from
`full_physical_formula_route_passed`. Each qualified case records either
`gpu_exercised` or `outer_unsat_without_membership`. The latter requires zero
membership candidates and completed UNSAT coverage; it does not count as a GPU
oracle exercise. Full physical-route qualification requires all 94 source and
answer contracts, valid route evidence for all 94 cases, and at least one
completed GPU membership batch across the campaign. A campaign consisting only
of outer-search UNSAT cases preserves its answer parity and per-case evidence
but fails the requested physical qualification. No dummy device work is added.
`formula_execution_cases` reports both populations explicitly. A physical
campaign remains a hybrid CPU/GPU result: exact residual search and outer
candidate generation still execute on the CPU.

The statistics parser checks the authored native CLI protocol and retains the
raw stderr. These are execution records from a trusted installed solver,
not hardware attestation. They do not supply GPU kernel timings, device memory
measurements or hidden model identities. Synthetic protocol tests validate
report decisions; only an actual device campaign establishes physical execution.

When the solver emits the optional phase section under `--stats`, each case also
records `native_phase_timings`. Host-monotonic integer nanoseconds and attempted
call counts remain separate from subprocess elapsed time and semantic work
counters. An unmeasured phase is `null`, distinct from an attempted zero-duration
phase. `complete: false` records timing-counter overflow; `complete: true` only
means that attempted timing counters are complete, not that every phase ran or
that solving completed. Source loading, statistics output and GPU kernel time
are outside this profile. Phase sums need not equal the driver interval.

The validator retains phase evidence even when the native invocation fails or
returns incomplete coverage. Missing sections remain absent for older solver
builds; malformed or out-of-range sections produce `native_phase_timing_error`.
The JSON evidence profile accepts `u64` counters without rounding. Neither a
missing measurement nor a malformed measurement changes answer parity or
physical-route qualification. `phase_timing_cases` counts available, complete
and malformed sections independently; timing studies must check those counts
and the semantic outcome before using a sample. Raw stderr is always retained.

The command exits 0 only when all 94 cases pass its requested mode, 1 when any
case fails, and 2 for setup/integrity/report errors. Refused source, incomplete
coverage, timeout, output limits, unsupported output and semantic mismatches
remain distinct failure categories. A reference-only pass always records
`full_native_target_passed: false`. It cannot establish native compatibility.

The corpus lockfile pins clingo 5.8.0. The first local run used the independently
installed clingo 5.8.2 and records that actual version; it is not labeled an
exact reproduction of the pinned solver environment. All 94 reference contracts
passed in that run, while the native full-target run returned 94 source refusals
and a failing exit status.

Linux/macOS child capture uses the reusable process contract above, with a
one-second initial cleanup interval and one further second if reaping remains
unresolved. The compatibility backend on other platforms retains the older
temporary-file/thread mechanism and its separate pipe-closure waits. Captured
reports scale with the fixed case count and configured per-child byte ceiling;
allocator and operating-system overhead are not included in that measure.

The upstream corpus remains MIT-licensed, Copyright (c) 2026 Gregory Gelfond;
its full notice is retained under `validation/corpus/kr-domains/LICENSE`.
