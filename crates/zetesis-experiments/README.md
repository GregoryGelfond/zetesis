# zetesis-experiments

This crate contains bounded development experiments, with reusable library entry
points and a `zetesis-bench` command adapter. It does not replace ordinary solver
qualification. Profiles are static reduct oracles (the default), `formula` for
general reduct checking, `formula-projection` for paired opt-in gate comparison,
`grounding` for fresh original-source formula admission, and `lazy` for matched
source-round measurements, and `tight` for complete-theory ranked certificates
with exact reduct completion.

Reproducible measurements of exact static reduct oracles. `zetesis-bench`
defaults to a physical Metal device and fails if one cannot be used. Select
`--backend cpu` explicitly for CPU measurements alone.

```sh
zetesis-bench --atoms 64,256 --batches 1,64,256 --repetitions 5 > metal.tsv
zetesis-bench --backend cpu --atoms 64,256 > cpu.tsv
```

Three deterministic rule families exercise forward chains, reversed chains,
and wide dependencies. Eight gate atoms provide 256 frozen interpretations;
larger batches repeat those patterns. Constraints mix accepted and rejected
candidates. Every measured result is compared with the exact dense CPU oracle,
including closure bits, constraint rejection, and seed mismatch. The dense CPU
oracle itself has independent exhaustive conformance tests against the lazy
oracle and subset-minimal semantics.

TSV output separates graph construction and static compilation, device and
worker-pool initialization, the first dispatch for each batch shape, and warm
dispatches. Warm GPU calls must reuse graph and transport buffers. A first call
for a new shape may retain graph storage from the preceding shape; the residency
record states which allocations occurred. The phase is named `initial-case`:
repeated dimensions may reuse both graph and transport. Timings include host packing,
transfers, execution and readback. Comparison runs outside timed regions.

These are microbenchmarks of the same explicitly compiled static program.
They exclude source parsing and complete candidate search. Scalar CPU, Rayon,
and Metal run in that order; thermal and ordering effects have not been
controlled. CPU allocations are included per call whereas Metal can reuse its
resident buffers. These limitations must accompany any reported measurements.
Use release builds. No speedup, energy benefit, or full-domain result follows
from portable tests or the CPU-only mode.

## Lazy relational source rounds

```sh
zetesis-bench lazy --backend metal --widths 4,8 --batches 1,32,128 \
  --workers 4 --warmups 2 --repetitions 12 > lazy-metal.jsonl
```

The library-owned `lazy_measurement::Configuration` selects finite cases and
independent candidate, source and transport limits. `measure` emits typed events
to a synchronous consumer; the command derives a JSON-lines view. Each run owns
one Rayon pool and, when requested, one physical Metal oracle. A missing device
fails explicitly. Use `--backend cpu` for the four portable routes alone.

Each fixture derives `a(I)`, `b(I)` and `c(I)` from frozen `pick(I)` choices and
joins `triple(X,Y,Z)`. A constraint rejects the all-zero triple. Sparse occurrences
select one value each, repeating after the configured width; dense occurrences
select every value. These are deterministic synthetic sources, with their source
construction supplied by the sealed executable. Sparse and dense populations
remain separate. The experiment does not enumerate the outer candidate carrier
or parse a source file.

The six physical-run routes are independent scalar checks, the same checks
scheduled by Rayon, portable Union rounds, portable Worlds rounds, Metal Union
rounds and Metal Worlds rounds. Each successful sample agrees with the same
complete scalar closures and rejection reasons in exact occurrence order.
Different routes may perform different source work; matching candidates does
not mean identical algorithms or resource charges. Scalar/Rayon limits apply
per candidate; source-round limits apply to the whole batch.

Preparation and initial observations are retained separately from warmups and
timed repetitions. Each population starts in the declared route order, then
rotates left by one position per iteration. Twelve repetitions balance all six
physical routes and all four CPU-only routes. Fixtures and references stay live
during samples. Pool/device setup is outside the intervals; complete source
scans, checking, result construction, transfers and readback are inside. Parity
comparison and JSON serialization are outside. Host wait is not shader time.

Source observations retain rounds, offered instances, source/mask work, pruned
prefixes and peak requested mask bytes. Metal observations require positive
submitted work consistent with the completed chunks and candidate occurrences.
Transfers are byte totals, not resident allocation or process RSS. Lazy source
state and transport are rebuilt per call; device/pipeline ownership alone is
reused. A route failure emits available source/device progress and returns a typed
error; it is not a successful timed sample. Setup/reference failures return an
error after the preceding events without a route-failure event. Neither failure
path produces a completion record. A writer can retain a prefix. Preserve stderr
and process exit with the JSON lines; failed attempts carry no elapsed-time claim.

The finite protocol permits up to 64 cases, width 16, 256 occurrences, six
warmups, sixty repetitions and 64 workers. Native limits can still refuse an
admitted dimension. Numeric configuration is emitted before setup. Record the
command, executable/source hashes, toolchain and host with the complete output;
the embedded library cannot identify its caller's executable or control thermal
state. Stop task-owned compute before timing. Portable tests alone provide no
physical Metal result or performance claim.

## Complete tight certificates

```sh
zetesis-bench tight --backend metal --atoms 4,64,256 --batches 1,32,128 \
  --families normal,choices --workers 4 --warmups 2 --repetitions 12 \
  > tight-metal.jsonl 2> tight-metal.stderr
```

`tight_measurement::measure` accepts a typed
`Configuration` and synchronous event consumer. `measure_with_control` also
accepts caller-owned cancellation/deadline control. The CLI is a JSON-lines view;
`--backend cpu` explicitly selects only scalar and Rayon classification. Physical
Metal is required otherwise, with no CPU fallback. No ordinary solver route is
changed or qualified by this experiment.

Every case owns one complete immutable original `Theory`, its checked
`TightPlan`, and the same ordered unfiltered candidate occurrences on every
route. The normal family is a fact followed by a positive implication chain.
The choices family has unconditional atomic choices except for its final
producer, which is guarded by the preceding atom when one exists. Both include one final atom
without any producer. Roots are deliberately reversed to exercise original-root
witness order. Candidates start empty, full, then full except the unsupported
atom; subsequent occurrences repeat eight-bit patterns across the atom carrier.
No candidate is filtered by its eventual result. Batch size one naturally
contains only the first occurrence. Larger batches retain direct stable
classifications, original nonmodels (normal family) and residual controls;
the choices family supplies a substantial directly stable population.

Before measuring a case, every occurrence receives an independent complete
membership reference. At most eight atoms use the finite Ferraris checker's
exhaustive proper-subset enumeration. Wider cases use the native general
original/frozen-reduct checker with no tight certificate enabled. Each reference
must complete. Scalar certificate witnesses are established separately, and
original failure roots are checked against the finite checker with subset work
disabled. Samples must match the reference status and exact certificate witness
for every occurrence. Every actual residual countermodel is independently
checked for theory identity, proper inclusion and satisfaction of the frozen
reduct. Different valid countermodels may be returned by the two reference
instruments; they are retained, not required to be identical.

The four routes are scalar certificate classification, indexed Rayon
classification, fresh Metal transport and resident Metal transport. All residuals
receive serial, occurrence-ordered exact native CPU completion on every route.
Rayon therefore parallelizes classification only. Whole-call elapsed,
classification and residual-completion intervals are directly measured with the
host monotonic clock; the latter two are nested within the first. Do not add them
together as separate costs or infer a GPU classification gain from blended total
time. Classification includes result allocation and, for Metal, packing,
transfers, execution, wait and readback. Residual completion includes its result
allocation, even with no residuals. These are not shader timestamps.

The run owns one Rayon pool and two independently initialized GPU oracles.
Reported adapter metadata must agree, which does not prove a unique physical
chip identity. Resource setup times and actual adapter metadata are emitted
outside sample clocks. Fresh residency is cleared before its clock starts;
its measured call must upload the theory and allocate transport. The other
oracle retains its graph and transport. Its initial observation primes that
case; later warm and timed calls must reuse both. Fixture/reference preparation,
parity/witness validation, fresh cache clearing, destruction and JSON publication
are outside sample clocks. Parsing, grounding, outer candidate generation,
objectives and complete solve time are excluded.

Initial, warmup and timed populations stay separate. Each starts in the declared
route order and rotates one position per iteration. Twelve repetitions balance
both the four-route physical schedule and the two-route CPU schedule. The command
above has 18 cases and 1,080 observations: 72 initial, 144 warmup and 864 timed.
CPU-only mode retains 540 observations. Configuration precedes setup and includes
all per-operation numeric limits. Prepared events retain the complete original
DAG, root order, producers, ranks, candidate identities and reference outcomes.
Sample events retain every actual outcome, direct CPU/GPU decision attribution,
exact residual counts and CPU certificate work. Device activity distinguishes
submitted/scheduled work from validated completed work. Authored transfer bytes
are not measured bus traffic; logical retained/scratch payload is not RSS.

An incomplete measured operation emits its failed position, measured prefix and
available activity, then returns an error. Setup/reference failures return an
error after their preceding prefix. Output failure preserves any written prefix.
No failure is replaced, and no completion event follows an error. Control is
polled before each resource setup, before publishing a successful sample, and
before final completion. A sample callback can cancel and retain that committed
sample, while preventing campaign completion. Cancellation after the completion
callback has committed is not retroactive. Retain stderr and process exit
alongside all JSON lines. The finite scope permits at most
24 cases, 256 atoms, 256 occurrences, six warmups, sixty repetitions and 64
workers; native limits can still refuse an admitted case. There is no implied
whole-campaign deadline or hard bound on pool/device creation or a caller's sink.

Use a release binary, retain its hash, source revision, command, host and
complete outputs, and stop task-owned compute before timing. Portable controls
include exhaustive tiny candidate/subset checks, witness substitution,
resource/cancellation failures and publication prefixes. Physical execution and
performance remain unqualified until a retained device campaign succeeds.

## General Ferraris formulas

```sh
zetesis-bench formula --backend cpu --cpu-workers 4 --atoms 64,256 --batches 1,64,256
zetesis-bench formula --backend metal --cpu-workers 4 --atoms 64,256 --batches 1,64,256
```

The separate `formula` profile measures original finite DAGs with choices,
cycles, conjunctions, disjunctions and masked implications. Resident GPU propagation returns
original-model rejection, proper-subset refutation or an explicit residual.
Every residual receives exact CPU membership checking inside the hybrid timing.
Native CPU results provide the reference for all candidates. Warm samples must
reuse both the immutable graph and the batch transport.

`--gpu-max-rounds` and `--gpu-max-work` bound propagation; a sweep limit leaves
an unresolved query. `--max-work` bounds exact CPU completion; exhaustion fails
the experiment. `dispatch_host_ns` includes packing, submission and readback,
not just shader execution. Each case runs the scalar native reference, an ordered
Rayon membership batch, then the hybrid GPU batch. `--cpu-workers` defaults to 4
and accepts 1–64; one explicitly owned pool is reused throughout the run. Pool
initialization is reported separately, with requested and actual workers.
The Rayon interval includes scheduling, native checking and result collection;
all workers join before an error is returned. Each query keeps its native limits
and shared cancellation control. GPU residual queries remain serial inside the
hybrid interval, so this is not a Rayon-completed hybrid measurement.
Candidate patterns repeat above 256. Source grounding, outer search and
objectives are excluded. The [design and qualification contract](../../docs/design/gpu-formula-propagation.md)
records remaining full-GPU work. Ordinary CLI formula solving can explicitly
select the same GPU primitive; this experiment alone does not qualify that
end-to-end integration. The dated M4 Pro record predates the Rayon baseline
and retains its original two-backend measurements.

## Paired gate projections

Build the engineering binary once, then use a quiet physical Metal session:

```sh
cargo +1.97.1 build -p zetesis-experiments --bin zetesis-bench --release --locked --offline
target/release/zetesis-bench formula-projection --backend metal \
  --atoms 3,64,256 --batches 8,64 --repetitions 4 --cpu-workers 4 \
  > target/gate-projection.tsv
```

With `CARGO_TARGET_DIR`, use its `release/zetesis-bench`. Record the revision,
binary SHA-256, toolchain, host and command with the raw output. The command fails
if physical Metal is unavailable; `--backend cpu` is explicitly refused. The
existing `formula` command and ordinary solver keep the Enumerated default.

The reusable `run_formula_projection(&FormulaOptions, &mut Write)` entry point
owns two independently initialized oracles, compares their reported metadata,
then checks both on the same original Theory instance and owned candidate batch.
Metadata equality cannot certify unique physical-device identity. Each sample
first obtains exact scalar and Rayon membership. Projection order is Enumerated,
Bitwise on even iterations and Bitwise, Enumerated on odd iterations, starting
with initial-case iteration zero. Both must preserve the exact native result for
every candidate and reuse their graph/transport during warm samples. The default
five formula families include shared children and false-masked implications.

TSV rows use `metal-with-cpu-residuals` for Enumerated and
`metal-bitwise-with-cpu-residuals` for Bitwise. `dispatch_host_ns` measures the full
host dispatch boundary, including packing, transfer, execution and readback;
it is not a device timestamp. Hybrid time includes serial native completion of
every residual. Residual and sweep counts can vary with device scheduling;
complete native membership must agree. Initial setup and warm rows remain
separate. Headers give actual numeric limits; residency Debug lines are
supplemental diagnostics, not the configuration schema.

These CPU quotas are per candidate. This command does not characterize ordinary
execution's cumulative shared quota, completion worker scheduling, outer
candidate stream, source grounding, objectives, or complete solve time. Shapes,
repetitions and per-query work remain bounded as for `formula`; the caller's
finite lists determine total case count. Any incomplete check, mismatch, device
failure or writer error stops the experiment without final PASS. A writer may
retain a prefix. The command retains no sample history itself.

Four warm iterations give each projection equal first/second positions, but do
not remove cache, thermal or driver effects. No Bitwise physical qualification or
speedup is established by its portable tests or by adding this command. Run the
separate explicit `zetesis-wgpu` `hardware_formula` tests to cover exhaustive tiny
frozen cases, exact resource boundaries, cache replacement and foreign identity.

## Original-source grounding

Build once, then run the binary in a quiet measurement window. From the repository
root, for example:

```sh
cargo +1.97.1 build -p zetesis-experiments --bin zetesis-bench --release --locked --offline
target/release/zetesis-bench grounding \
  examples/kr-domains/standalone/send-money/send-money.lp \
  --repetitions 3 > target/send-grounding.json
```

If `CARGO_TARGET_DIR` is set, use that directory's `release/zetesis-bench` instead.
Keep the command, source revision, toolchain, executable SHA-256 and raw JSON
together. The report records the exact original file SHA-256 values and all
numeric native/capture limits; it cannot identify the executable or host of a
caller embedding the library. Raw captures belong in ignored target storage.

The library entry point is `grounding::profile(path, Configuration)`. It loads the
original include graph, makes one unmeasured reference admission and complete
native enumeration, then runs three conditions per round. Round zero uses
`unobserved`, `boundary`, `detailed`; subsequent rounds rotate that order. Every
condition reloads and parses the original files before timing, checks their exact
bytes and include identities, and uses the same native limits. The reference
subject stays live during all samples. These are repeated fresh admissions in
one process, with source pages already accessed, rather than cold process starts.

`admission_elapsed_ns` starts after loading/parsing, source comparison and observer
record preallocation. It covers native raising, normalization, analysis and
actual grounding through the returned admitted subject. No observer is passed in
`unobserved`, so `grounding_elapsed_ns` is null. The other modes time the existing
actual-ground enter/exit boundary. Only `detailed` retains phase-local work and
phase durations, with original source byte spans for per-rule instantiation.
Repeated expanded rules may share a span. Rule instantiation still interleaves
joins, comparisons and formula emission; it is not arithmetic-only time. Selected
work counts are not a count of every operation, allocation or byte.

After each timer, the driver compares complete ordered atom/node/root catalogs,
metadata, formula origins and objective origins/templates/declarations. It then
uses native stable-model enumeration with an optional complete-theory tight
certificate, falling back to the exact reduct route when ineligible. Search,
certificate setup, comparison, model copying and sorting are outside admission
timing. The JSON atom catalog contains full signed identities; each model is a
vector of true atom indices into that catalog. Hidden atoms remain present and
model multiplicities are retained. Native exhaustion is required, including for
an inconsistent source with zero models. This internal parity is not independent
clingo agreement, a compiler proof, or qualification of a physical backend.

The initial reference and each sample also retain `subject_fingerprint`, an
additive schema-one field for comparisons between executables. Available records
identify the `zetesis-execution-subject-v1` framed SHA-256 grammar and byte count.
The [encoding contract](../../docs/verification/scalar-evaluation-20260907/fingerprint.md)
specifies complete ordered atoms/values, formula nodes/roots, grouped provenance,
objective absence and display metadata. Hashing occurs outside timing with
fixed-size scratch and an independent byte-work ceiling. A matching fingerprint
is collision-resistant evidence, not a proof of identity or an exportable program.
Term-observation query plans have no complete public representation and produce
an explicit `unavailable` reason, while exact internal comparison and complete
enumeration continue. `complete: true` alone does not imply an available digest.
Cross-executable ablations must require an available matching grammar/digest for
every reference and sample, identical source/configuration seals and complete
matching full-model multisets. They must retain all measured samples.

Objectives, including inactive/empty declarations, are explicitly refused; this
experiment does not silently enumerate past an optimization contract. Native
admission/search refusals and capture exhaustion yield `complete: false`, retain
available phase/model prefixes, and make the command exit with status 2. A phase
record ceiling never changes native grounding; it refuses the experiment after
the attempted admission. Native failure and capture refusal remain separate.
There is no panic recovery or hard wall-clock deadline; each native operation
has its finite resource limits and external campaigns can impose a process limit.

The command accepts 1–11 rounds and these independent inclusive capture ceilings:

| Option | Default | Counted resource |
| --- | ---: | --- |
| `--max-phase-records` | 4096 | Records per detailed admission |
| `--max-models` | 256 | Retained models per enumeration |
| `--max-model-atoms` | 65536 | True-atom indices across one enumeration |
| `--max-atom-text-bytes` | 1048576 | Full catalog spelling bytes |
| `--max-source-path-bytes` | 1048576 | Retained native source path bytes |
| `--max-subject-bytes` | 67108864 | Framed execution-subject bytes hashed per admission |
| `--max-output-bytes` | 16777216 | Entire JSON record and newline |

Zero is a real ceiling. The command uses unchanged native defaults; the public
`Configuration` exposes all native limits for controlled library experiments.
The JSON schema serializes every configured numeric field through reporting-local
views; adding a native limit requires updating those views. Report serialization
preflights the byte ceiling before touching an external writer. Writer failure
can retain a byte prefix and never returns success. `write_report` leaves the
typed report available to its caller.

This driver measures the formula eager path only. It supplies no evidence about
relational full-carrier eager grounding or the separate relational lazy engine;
SEND is not a supported lazy input. Compare detailed records to locate work, then
use unobserved conditions for performance claims. Boundary-only and detailed
records expose observer overhead, but rotating order does not eliminate thermal,
allocator, cache or scheduling effects. No evaluator optimization or measured
improvement is established by adding this driver.
