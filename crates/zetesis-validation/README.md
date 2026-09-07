# Validation tooling

This package has two commands and reusable corpus, process-capture and reported-answer libraries. `zetesis-corpus`
checks the selected clingo fixture's integrity without running a solver.
`zetesis-validate` runs the separate 94-case kr-domains solver campaign described
below. Neither command is invoked by the production solver.

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
the older command's path, and is separate from the existing Python upstream
campaign's POSIX requirement.

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
original bytes. The selected 24-case Rust solver campaign remains follow-on work.
The old Python comparison and retained C++ import
fixtures are still required by their existing callers.

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

Both commands emit machine-readable reports. `zetesis-corpus` writes one JSON
record only after successful verification, with `semantic_solver_run: false`.
It exits 0 on integrity success and 2 on argument, integrity, filesystem or output
failure. Diagnostics go to stderr. A stdout write failure can leave a partial
record; consumers must check successful process completion before accepting it.

The five Rust source/ordinary-solver regression consumers now read this owned
curated corpus directly. The existing Python comparison/gate still reads
`cases.jsonl`; the explicit import tests still read the retained C++ originals.
Those legacy inputs remain until their callers and independent checks are migrated.
See the [migration record](../../docs/verification/corpus-curation-20260907/README.md)
for the exact remaining work. This first slice does not replace the solver
campaign or claim repository cleanup is complete.

## Full kr-domains regression gate

`zetesis-validate` is an independent validation executable. It is not linked to
or invoked by the production solver. The default target contains all 94 original
non-clingcon cases and their 14 schema dependencies at revision
`38f0660ded448ed268c5a68759ceb0e2840dd497`. It reads the vendored corpus and its
manifest from the selected repository; no network access or estate checkout is
needed.

```sh
zetesis-validate --repo /path/to/zetesis --report validation-report.json
zetesis-validate --repo /path/to/zetesis --reference-only --report reference-report.json
zetesis-validate --repo /path/to/zetesis --native-backend metal --native-oracle countermodel --native-batch-size 64 --report fresh.json
zetesis-validate --repo /path/to/zetesis --native-backend cpu --native-completion-workers 4 --native-max-completion-scratch-bytes 268435456 --report cpu-batched.json
```

The installed validator and solver run directly through PATH. `--corpus` and
`--manifest` override those paths. `--clingo` and `--zetesis` select executable
paths or names resolved through PATH. A relative executable path is resolved
relative to the caller's working directory, not the corpus. `--help` lists
resource limits. Omitting `--report` writes the JSON report to stdout; progress
goes to stderr.

The exact manifest SHA-256 is pinned in the validator; changing its whitespace,
paths, hashes, includes or contracts requires a deliberate trusted-target update.
Every referenced source is SHA-256 checked before any child is spawned.
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
The elenctic test expectations `@expect`, `@cost`, `@count`, `@model`, `@optimal`
and `@cautious optimal` are checked by the validator; unknown expectation tags
fail explicitly. They are ASP comments to the solver, not native source-language
semantics. Notes are prose.
Strings with spaces, commas and escaped quotes remain whole atom values.
Within one display, symbols are sorted and every occurrence is retained: an atom
and a shown term can print the same symbol twice. Hidden atom identities
cannot be reconstructed from `#show`; this corpus contract does not establish
them. The optimization-aware timing companion uses the same rule for repeated
symbols within each display.

Native runs default to `zetesis --backend cpu --oracle auto --models 0 --batch-size 64
--completion-workers 1 --max-completion-scratch-bytes 268435456` on each complete
original source graph. The current full campaign passes all 94 cases under the
default limits. These results cover the pinned non-clingcon corpus and its
original contracts; broader clingo language coverage is a separate target. Optimized
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
