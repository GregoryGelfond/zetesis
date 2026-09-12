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
| `relation` | Equality masks over one retained typed column view | `relation_measurement` |
| `table` | Complete surviving rows and projected domains on CPU | `table_measurement::measure` |
| `grounding` | Fresh original-source formula admission | `grounding::profile`, `write_report` |

Device profiles accept physical Metal or Vulkan. CPU mode explicitly omits the
device; `formula-projection` requires a physical device, and `grounding` is
CPU-only, as is `table`. An explicit API never silently falls back to another API, CPU or a
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
zetesis-bench relation --backend metal --family independent --payload tuple \
  --rows 4096 --queries 32 --workers 4 --warmups 2 --repetitions 6
zetesis-bench table --case aliased --rows 1024 --queries 32 \
  --workers 4 --warmups 1 --repetitions 3
zetesis-bench grounding examples/kr-domains/standalone/send-money/send-money.lp \
  --repetitions 3
```

Use each profile's `--help` for its independent dimensions and resource limits.
Redirect stdout to retain the report and stderr to retain diagnostics; also
record the command's exit status. Reports are profile-specific TSV, JSON or
JSON-lines views, not a single interchangeable timing schema.

## What each measurement includes

### Finite-table domain projection

`table` compares a complete typed row scan, scalar support bitsets and an owned
Rayon pool sharing one immutable support index. Every procedure returns the same
original row occurrences and projected domains. The scan prepares sorted domain
memberships once; table projections resolve their supplied typed domains on each
call. Their distinct setup costs are recorded. The independent reference scans
original rows with linear domain membership and pairwise alias checks.

The fixed cases are correlated numeric columns, independently varying mixed typed
columns and repeated-variable columns with both coherent and incoherent rows.
An explicit source-index map retains duplicate occurrences in reverse source
order. Seven query families repeat in order: full domains, singleton domains,
absent values, alternating half domains, an empty first domain, the complementary
half domains and restored full domains. Restoration tests reuse the same table.
These are finite relation constraints; they do not establish ASP producer support,
source completeness, candidate truth or answer-set membership.

JSON-lines events contain the complete typed subject and its SHA-256, one-time
preparation intervals, table work/capacity receipts and every initial, warmup and
timed batch. Output domain IDs refer to the subject's typed `values` list, not
logical integers or relation dictionary IDs. Original row positions retain their
`original_indices` mapping. Each batch is checked outside its timing interval;
per-query projection and conversion intervals are also separate. Use batch wall
times when comparing scalar and Rayon schedules; summing overlapping worker
intervals does not give parallel wall time. Routes run in fixed scan/table/Rayon
order, so measurements do not control order or thermal effects.

The library caps dimensions at 8192 rows, 128 queries, four variables, seventeen
typed dictionary values, eight workers and five timed repetitions. Preparation
and per-query table work have independent receipts under the requested shared
per-operation ceiling. Limits cover the relation and table input/scratch for
that operation; caller fixture, other retained results and thread stacks are
separate. The report retains a conservative fixture envelope and actual common
result-vector capacities. These measures are not RSS. The CLI stops publication
at 64 MiB. Exit 0 means every scheduled comparison completed without refusal;
exit 1 retains a completed schedule with finite-table applicability refusals;
exit 2 denotes a command, acquisition, correctness, interruption or output failure.
Missing metrics are omitted, never represented by zero.

No GPU procedure or ordinary solving path is selected by this command.
Singleton equality-mask comparisons belong to the distinct `relation` contract;
they must not be compared with timings that also compute projected domains.
Keep source revision and executable hash beside reports when measuring changes.

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
GPU work counts complete node, root, producer and atom scans plus initialization
of `ceil(atoms / 32)` support words per candidate. Scalar early-exit work uses
different charges. Reported transport bytes include the packed support buffer;
they are logical requested payload, not process RSS or total device memory.
`--support atomic` keeps independent atomic producer writes; `--support grouped`
selects one complete reduction per support word. The configuration records this
selection. Both preserve the same complete producer occurrences and independently
checked witnesses. Grouped construction adds word offsets and temporary packing
cursors, with exact admission accounting; a smaller number of atomic updates
does not establish faster execution. This option belongs to the primitive
benchmark, not the ordinary solver command.
The explicit `support-uniform` and `support-skewed` families keep the same choice
DAG and candidate sequence. Both retain sixteen producer occurrences per
supported atom in total. Uniform distributes repetitions evenly; skewed retains
each original once and concentrates the remaining occurrences on one head.
Their separate limits are 4096 atoms, 1024 candidates and 65,520 producers.
Normal and choices retain their original 256-atom and 256-candidate limits.
Prepared events retain the complete occurrence sequences; timings from these
larger families require separate resource and measurement qualification.
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

### Retained relation selection

The relation profile constructs one bounded typed source, its equality dictionary
and aligned columns. Scalar CPU, Rayon and the requested physical GPU receive the
same ordered queries and produce identical packed row masks. An independent
comparison against the original typed rows checks each complete output. All
routes then reconstruct selections through the same core operation and traverse
every selected argument cell.

Source construction, dictionary construction, query resolution, input selection,
Rayon pool creation, device setup and column upload are reported separately.
Repeated batches reuse the immutable relation, resolved queries and uploaded
columns. Scalar and Rayon construct packed masks directly through the core
equality predicate, then copy them into the common batch. GPU selection includes transfer, readback
and the copy into the common mask representation. Reference checks, source
hashing and report publication lie outside operation clocks. Routes run in fixed
scalar/Rayon/GPU order; order and thermal effects remain uncontrolled.
Validation traverses the rows before timed reconstruction, so that interval is
not a cold-cache observation. In physical mode, uploaded columns remain resident
during CPU samples as well.

The authored storage bound includes shared views, packed outputs and concurrent
temporary masks; later reference validation and reconstruction charge their
position vectors separately. It is neither process RSS nor total device memory. Report
events retain the full typed subject, its hash, complete masks and actual device
work. Complete evidence requires both the final event and successful execution
and publication. A writer failure can leave bytes from an incomplete event;
the caller remains responsible for flushing its output.

Relation report schema 2 records direct CPU mask production. Its
`cpu_selection_work` charges mask-word zero writes, visited rows, short-circuit
equality comparisons and selected-bit writes. Schema 1 counted the position
producer before a separate packing loop. These counters use different work
conventions; complete typed subjects and mask membership remain comparable.
Neither counter includes the common batch's initialization or mask copy.

This profile measures equality selection and typed row access. It does not
measure full tuple matching, source grounding or answer-set solving. See the
[library contract](src/relation_measurement.rs),
[shared fixtures](src/relation_fixtures.rs) and
[regressions](tests/relation_measurement.rs).

### Original-source grounding

The grounding profile loads the original include graph and performs unmeasured
indexed-join reference admission and complete native solving. It then rotates fresh admission
with no observer, boundary observation and detailed observation. File loading
and initial parsing lie outside the admission timer. Admission includes native
raising, normalization, analysis and materialization; observer grounding spans
isolate finite instance construction.

`--joins indexed` selects the existing shortest-posting joins. `--joins table`
selects reusable support masks for eligible flat positive patterns over completed
possible support. Every timed strategy is checked against the indexed reference;
the report records both choices. Detailed counters distinguish preparation,
reuse, actual table/indexed probes, query work and accounted support peak. Other
contexts retain indexed matching, and resource exhaustion remains a failure.
Compare preparation and query costs together before inferring a grounding gain.

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
cargo test --locked -p zetesis-experiments --test relation_measurement metal -- --ignored --nocapture
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
