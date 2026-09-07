# zetesis-experiments

This crate contains bounded development experiments, with reusable library entry
points and a `zetesis-bench` command adapter. It does not replace ordinary solver
qualification. Profiles are static reduct oracles (the default), `formula` for
general reduct checking, `formula-projection` for paired opt-in gate comparison,
and `grounding` for fresh original-source formula admission.

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
  validation/corpus/kr-domains/standalone/send-money/send-money.lp \
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
