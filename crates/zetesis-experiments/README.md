# zetesis-experiments

Bounded development experiments with reusable Rust interfaces and a
`zetesis-bench` command adapter. Each profile names the operation it measures,
its reference comparison and its limits. Primitive measurements do not establish
ordinary solver acceleration.

Use [Install and run](../../README.md#install-and-run) to install a release build.
The [execution chapter](../../docs/book/architecture/execution.md) describes how
these operations fit into the solver. Public interfaces start in
[src/lib.rs](src/lib.rs); generate their reference with
`cargo doc --locked -p zetesis-experiments --no-deps --open`.

## Choose a profile

| Profile | Operation | Library entry |
|---|---|---|
| Default | Static reduct closure over deterministic compiled rules | `run`, `Options` |
| `formula` | General formula membership with exact residual completion | `run_formula`, `FormulaOptions` |
| `formula-projection` | Paired exact gate-projection strategies | `run_formula_projection` |
| `aggregate` | Numeric reductions over matched original/frozen eligibility | `aggregate_measurement` |
| `tight` | Complete-theory tight certificates with exact completion | `tight_measurement` |
| `lazy` | Matched relational source rounds and closures | `lazy_measurement` |
| `grounding` | Fresh original-source formula admission | `grounding::profile`, `write_report` |

Device profiles accept physical Metal or Vulkan. CPU mode explicitly omits the
device; `formula-projection` requires a physical device, and `grounding` is
CPU-only. An explicit API never silently falls back to another API, CPU or a
software adapter. Device metadata records selection, not platform-wide
qualification. The default static command requires Metal.

```sh
zetesis-bench --backend cpu --atoms 64,256 --batches 1,64,256
zetesis-bench formula --backend metal --cpu-workers 4 \
  --atoms 64,256 --batches 1,64,256
zetesis-bench aggregate --backend metal --tuples 0,64,4096 \
  --batches 1,32,128 --functions count,sum,sum-plus,min,max \
  --workers 4 --warmups 2 --repetitions 12
zetesis-bench tight --backend metal --atoms 4,64,256 \
  --batches 1,32,128 --families normal,choices --workers 4
zetesis-bench lazy --backend metal --widths 4,8 \
  --batches 1,32,128 --families sparse,dense --workers 4
zetesis-bench grounding examples/kr-domains/standalone/send-money/send-money.lp \
  --repetitions 3
```

Use each profile's `--help` for its independent dimensions and resource limits.
Redirect stdout to retain the report and stderr to retain diagnostics; also
record the command's exit status. Reports are profile-specific TSV, JSON or
JSON-lines views, not a single interchangeable timing schema.

## What each measurement includes

### Static and general reduct checking

The default profile compares deterministic chain/dependency families with the
dense CPU reference, including closure bits and rejection reasons. Repeated
candidate patterns remain repeated occurrences. Setup, initial-case dispatches
and warm dispatches are reported separately; warm GPU calls must reuse declared
graph and transport buffers.

Static measurements exclude source parsing and complete candidate enumeration.
Scalar, Rayon and GPU routes run in fixed order, so thermal/order effects are
uncontrolled. CPU allocations can be timed while GPU buffers remain resident.
Keep these differences with any comparison.

The formula profile uses the same ordered finite candidates for scalar CPU,
Rayon CPU and hybrid GPU checking. Every GPU residual receives exact CPU
completion within hybrid time; a propagation limit cannot turn a residual into
an accepted answer. The benchmark's residual completion is serial, unlike the
ordinary solver's optional parallel completion. Source grounding, outer search
and objectives are excluded.

`formula-projection` compares Enum and Bitwise projection with alternating pair
order. Each route owns initialized device resources and must preserve ordered
exact results and declared residency. Equal adapter metadata is not proof that
two separately created devices select the same physical adapter.
See [formula measurement](src/formula_measurement.rs) and
[projection tests](tests/formula_projection.rs).

### Native aggregate reductions

The aggregate profile acquires original/frozen eligibility over complete tuple
keys, then compares exact scalar, Rayon and GPU reductions. Preparation reports
Group/Theory construction, mask acquisition, wire preparation and reference work
separately. Acquisition and its retained mask storage must accompany a
reduction-only performance claim.

`device-fresh` clears residency before each sample but retains initialized
device/pipeline state. `device-resident` primes and reuses the group's transport.
Both perform new reductions. Route positions rotate; parity checks, publication
and explicit residency clearing lie outside sample clocks.
These labels describe allocation/upload boundaries, not process-cold execution.

The primitive preserves original/frozen measures and guards. It does not
establish source completeness, head permission or stable-model minimality.
See [aggregate API](src/aggregate_measurement.rs),
[fixture construction](src/aggregate_measurement/fixture.rs) and
[regressions](tests/aggregate_measurement.rs).

### Tight certificates and lazy source rounds

The tight profile compares complete original theories and identical unfiltered
candidate occurrences. Certificate preparation and independent reference checks
are outside sample clocks. Exact CPU completion of every residual is inside.
Small references use exhaustive finite membership; larger cases use general
checking without the tested certificate. Fresh and resident device routes keep
their separate allocation contracts.
See [tight API](src/tight_measurement.rs) and
[checking tests](tests/tight/checking.rs).

The lazy profile compares independent scalar/Rayon source joins, portable shared
Union/Worlds rounds and corresponding GPU routes. CPU-only mode uses the four
portable routes. Ordered closures and rejection reasons must match even when
routes charge different work. Source-round execution, transport and returned
results are timed; setup, reference comparison and rendering are separate.

These source fixtures are generated and bounded. Measurements include neither
external parsing nor an ordinary outer candidate search.
See [lazy API](src/lazy_measurement.rs) and
[regressions](tests/lazy_measurement.rs).

### Original-source grounding

The grounding profile loads the original include graph and performs unmeasured
reference admission and complete native solving. It then rotates fresh admission
with no observer, boundary observation and detailed observation. File loading
and initial parsing lie outside the admission timer. Admission includes native
raising, normalization, analysis and materialization; observer grounding spans
isolate finite instance construction.

After timing, each result is checked against the reference atom/formula catalog,
source evidence and complete native model collection. A fingerprint is available
only when its full specified identity view can be formed; unavailability is
explicit. The currently unavailable full term-observation fingerprint must not
be replaced with a weaker hash and called equivalent.

Objectives, including inactive declarations, are refused by this profile.
It measures eager formula admission, not lazy grounding. Detailed rule attribution
can blend joins, comparisons and formula emission; it is not standalone
arithmetic-kernel timing. Finite work/storage limits are not a hard wall-clock
deadline. See [grounding API](src/grounding/mod.rs) and
[regressions](tests/grounding.rs).

## Interpret evidence and failures

The measurement libraries accept typed configurations and expose typed results
or synchronous event consumers. Preparation, sampling, parity checks and
publication have separate boundaries. Failed attempts retain their position and
available accounting; there is no replacement sample or success claim after a
failure. A writer can retain an already published prefix.

Host GPU-call intervals include packing, transfers, submission, waits and
readback. They are not device timestamps or shader time. Retained logical masks,
active transfers, authored device allocation and process RSS are different
quantities; overlapping payload scopes must not be added.

Resource limits separately bound fixture dimensions, native work, GPU work,
transport, reference checking and output. Cancelled or bounded computation is
not silently treated as an empty result. A complete measurement record is also
not a proof of ordinary solve coverage.

Portable tests exercise reference agreement, order, resources and publication
failure. Physical tests deliberately remain opt-in:

```sh
cargo test --locked -p zetesis-wgpu --test hardware_aggregate metal -- --ignored --nocapture
cargo test --locked -p zetesis-experiments --test aggregate_measurement metal -- --ignored --nocapture
```

Use `vulkan` in place of `metal` for these controls on a suitable host.
Missing hardware fails the requested physical tests; portable success does not
qualify a device. Additional maintained suites cover
[backend selection](tests/backend_selection.rs),
[formula completion](tests/formula.rs),
[tight execution](tests/tight_measurement.rs),
[grounding limits](tests/grounding.rs) and
[failure contracts](tests/failure_contracts.rs).
For complete programs and cross-solver timing, use
[zetesis-validation](../zetesis-validation/README.md).
