# Ordinary solve phase measurements

The purpose of measurement is to select general execution and representation
improvements while keeping the original reduct authoritative. A completed model
is still accepted by the existing semantic procedure; a timer does not change
candidate restrictions, work limits, completion or objective decisions.

`--stats` enables host-monotonic timing. The CLI composes a fixed-size recorder outside
its fallible driver result with the reusable `zetesis-telemetry` stage recorder. Failed admission, checking, scoring or output attempts
can therefore retain their elapsed intervals when the diagnostics sink works.
This does not recover a semantic partial report lost on a late error. Without
statistics there are no clock reads or timing-specific heap allocations; optional
branches and fixed-size fields remain.

| Phase | Timed operation |
| --- | --- |
| `admission_materialization` | Source/bundle admission and finite materialization, including a failed automatic closure-to-formula retry. |
| `execution_setup` | Engine, worker pool or device creation, including deferred setup/fallback. |
| `candidate_setup` | Seed generator creation or initial classical formula encoding. |
| `certificate_setup` | Complete original-theory certificate construction, including refusal. |
| `certified_membership` | Ranked-support checking, including interrupted attempts. |
| `candidate_generation` | Seed batches or classical queries, projection and exact semantic blocking, including the final exhausted query. |
| `original_validation` | Independent original-formula evaluation, including residual prechecks. |
| `gpu_host_oracle` | The actual GPU oracle host call, including its packing, transport, waiting and readback. |
| `exact_reduct_membership` | Frozen-reduct encoding, exact native search and returned countermodel validation. |
| `closure_membership` | Existing CPU closure batch, including lazy joins and worker scheduling/collection. |
| `objective_scoring_retention` | Objective evaluation, comparison and bounded original-model retention. |
| `objective_feedback` | Objective plan construction and candidate-only bound compilation/application. |
| `observation_output` | Observation queries, rendering, publication and result summaries. |

The driver interval starts at admission entry and ends when the operation returns.
Source loading and statistics emission are excluded. The phase intervals do not
nest, but do not partition the entire driver: retry preparation, formula-model
conversion and queue/accounting orchestration can lie outside them. Timer
bookkeeping is not separately measured and can occur inside a recorded interval.
The legacy phase footer retains its historical `timer_overhead=unattributed`
label for wire compatibility; the new stage footer states `not_separated`.
Closure membership includes the conversion/collection performed by its existing
executor, so the two routes do not claim identical internal decompositions.
The detailed admission phase combines source preparation and materialization.
The separate coarse stage profile below isolates explicit grounding. Lazy joins
can occur during membership, and GPU host time is not shader time.

An attempted call can take zero at the clock's resolution. An unentered or
inapplicable phase is printed as `unmeasured`, not zero. Counts refer to calls that
returned, including failures; they need not equal semantic query or model counts.
Checked measurement overflow retains a prefix and marks it incomplete, without
altering semantic resource accounting. Source cancellation or an error remains
visible in the ordinary result protocol. Failure of the diagnostics sink can
prevent the timing section from being written.

The native search API exposes optional `SearchPhaseTimings`; enabling it is
idempotent and covers future calls only. The CLI copies cumulative search snapshots
by replacement rather than adding them repeatedly. Successful `Report` values
carry optional `PhaseTimings`; error diagnostics carry attempts without claiming
a recovered semantic ledger. These APIs add no effect framework or alternate
acceptance path.

`zetesis-validate --native-stats` records an optional structured phase section
alongside raw stderr for each original corpus case, including failed runs. The
reader refuses duplicate, malformed or out-of-range measurement fields without
changing the separate answer-parity decision. Its integer JSON evidence profile
uses `u64` nanoseconds; a wider native duration is an explicit measurement error.
Every timing study must check phase availability/completeness and semantic
completion, rather than interpreting validator success as complete measurement.

The first experiment profiles the original 94-case corpus through ordinary CPU
formula solving. This identifies phase costs; it is not yet a GPU crossover or
instrumentation-overhead experiment. Subsequent matched candidate-stream and
full-solve studies should retain cold starts, exact residual work, objective
feedback, all failures and best relevant CPU baselines. A lazy/factored execution
experiment additionally needs growth in attempted bindings, retained relations,
formula size and peak memory. Phase time alone cannot show avoided grounding.

## Separate grounding and solving stages

An additive schema-1 stage section precedes the existing schema-2 phase section.
It is a separate exclusive host-wall profile, not a sum of detailed phase or
worker measurements. Four stages and uncovered time partition the recorder's
interval when nesting and duration arithmetic are valid:

| Stage | Scope |
| --- | --- |
| `source_preparation` | Source/bundle parsing, admission, preparation and retry work outside explicit finite grounding. |
| `grounding` | The actual eager relational `GroundProgram::compile` or frontend `formula_ground::ground` call, including failed attempts. Reusing a cached ground program does not create another grounding measurement. |
| `solving` | Setup, candidate generation, checking, coordinator waits, scoring and objective work, excluding nested grounding and observation/output. |
| `observation_output` | Model observation, rendering and publication, including attempted failures. |
| `unattributed` | Host intervals outside these stages. Instrumentation overhead is not isolated; some bookkeeping occurs inside measured intervals. |

`grounding_mode` is `unentered`, `eager`, `lazy_interleaved` or `mixed`.
Lazy source joins have no separate grounding duration and remain inside solving;
the output says unavailable, not zero. Mixed execution retains measured eager
spans and states that lazy work remains interleaved. Early failure can leave a
stage unentered. Stage counts describe entered attempts, not completed logical
operations. Timing integrity is distinct from semantic completion and optimality.

`zetesis-telemetry` owns the clock, RAII guards and typed `StageTimings`. Its caller
defines the interval; it imports no solver, frontend or CLI. The frontend's
optional `GroundingObserver` is clock-free and brackets only finite formula
grounding. CLI adapters select the observer and render a friendly millisecond
summary plus exact nanosecond fields. `PhaseTimings.stages` exposes the same typed
snapshot without parsing the text. This does not complete the separate extraction
of ordinary solve orchestration from the CLI.

The validator accepts an absent stage section for legacy output. When present,
it checks the exact schema, availability, counters, complete exclusive sum and
arithmetic bounds. Its `native_stage_timings` evidence and stage availability /
integrity totals do not change answer parity or device qualification. Host elapsed
includes waits, excludes source file loading and statistics emission, and is not
GPU kernel time. No extra clock reads occur when statistics are disabled.

With `--json`, both timing views appear as typed fields in the terminal statistics
object. The JSON document header precedes the recorder and its terminal envelope
follows the snapshot; both are excluded explicitly. Individual model evaluation,
encoding and emission remain measured output work. This avoids claiming that a
terminal record includes the time needed to emit that same record.
