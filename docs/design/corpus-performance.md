# Reproducible kr-domains performance matrix

Status: required comparison, with runner migration and execution pending. The
existing three-case CPU measurements are not this matrix. This protocol applies
the broader [evaluation contract](zetesis.md#required-cpugpu-evaluation-contract)
to every original non-clingcon kr-domains input in the sealed corpus manifest
(currently 94 cases).

## Requested configurations and actual execution

For each case, compare the same source/include bytes, constants and semantic
task across these five primary configurations:

| Solver | Grounder requested | Backend requested |
| --- | --- | --- |
| zetesis | eager | CPU |
| zetesis | lazy | CPU |
| zetesis | eager | Metal |
| zetesis | lazy | Metal |
| clingo | native clingo grounding | CPU |

Pin both zetesis worker counts (closure and formula completion) in the campaign
manifest; the first primary campaign uses four for each and one clingo worker.
Report the unequal host-worker allocation explicitly. Add separate one-worker
zetesis and four-worker clingo series so readers can assess scalar and parallel
costs. An automatic-route series may supplement the explicit matrix, but may not
replace any cell or conceal a fallback.

Every case/configuration has a retained disposition: complete and parity-checked,
unsupported source/combination, unavailable device, timeout, resource stop,
parity mismatch, or execution/capture/cleanup failure. Record requested and actual
grounder, oracle, adapter and work. A successful Metal request with zero device
work is a completed run with zero device work, not evidence of GPU acceleration.

At the starting `8ce3191` checkpoint, explicit lazy source joins accept CPU or
auto and refuse explicit Metal. Some formula sources also refuse lazy grounding.
Those cells must appear as unsupported until implementation and qualification
change that fact. Never relabel eager fallback as a lazy run. No speedup ratio is
defined against an unsupported, incomplete or failed cell.
Removing the blanket lazy/Metal refusal through qualified lazy device execution
is required before version 1.0; this pending state is not an intended product
restriction.

## Comparable semantic tasks and output

Use exhaustive answer-set enumeration for non-optimization programs and complete
optimal-tie enumeration with proved optimum for optimization programs. Retain
UNSAT completion separately from a stopped search. Record canonical complete
models, original display multiplicities, objective priorities/vectors and all
optimal ties. A single model or matching model count does not discharge parity.

Use typed machine output where qualified; preserve the original source's display
contract. Native full-model records must not be compared to clingo's projected
symbols and called full-model parity. Where clingo's ordinary output hides atoms,
obtain a separately qualified full-model capture channel using the original
source, or explicitly label full-model parity unavailable while retaining the
display/count/cost comparison. Do not rewrite away `#show` for a timing claim about
unchanged original inputs.

## Timing and memory boundaries

Each primary cell has one first-observed invocation, three warmups and twenty
timed repetitions. Rotate the five configurations so each occupies every timed
position four times. Preregister case order, parameter values, resource limits,
capture/storage ceilings and the schedule in an immutable campaign manifest.
Classify deterministic unsupported combinations in a retained admission census;
they do not need repeated refusal timing. Keep scheduled positions fixed for
remaining configurations. Preserve every launched sample, including warmups,
failures and interruptions; a bounded campaign may resume only against the same
sealed inputs, executables and protocol, with its separate session recorded.

Measure inclusive process wall time through output capture and reaping. Validate
captured semantics outside that interval. Report raw samples, medians and spread,
not only winners or one aggregate speedup. A first-observed invocation is not a
controlled cold-cache run. Fresh one-shot Metal timing includes adapter discovery,
shader/device setup, transfers, waits and exact host residual work. Any resident
measurement is a separate experiment with its amortization stated.

Collect `--stats` phase observations in a separately labeled instrumented series
on the same executable and configurations, preserving its overhead. Eager source
loading, preparation, grounding and solving are separate where the current timers
actually distinguish them. Lazy grounding is interleaved with solving: report
that scope and available subphase observations instead of inventing an isolated
grounding duration. CPU worker time and wall time are different measurements;
host GPU dispatch/readback timing is not device kernel timing.

Collect process peak RSS, host allocation accounting, GPU allocation accounting
and transfer bytes where supported, with units and measurement boundaries.
Unavailable values carry a reason. Unified-memory quantities can overlap and
must not be added as disjoint memory use. This is essential for interpreting
eager/lazy tradeoffs even when their times are similar.

## Parameterized N-queens series

Extend the original-input matrix with a separately identified size sweep. In the
current corpus, variants 02–06 declare `#const n=8`; variant 01 hardcodes
`row(1..8)` and `column(1..8)`. Keep variant 01 as an original eight-queens case.
Any generalized version is a derived fixture with its own provenance and hash,
not another unchanged original input.

The current zetesis CLI has no constant-override option. A library-owned
[source parameter configuration](source-parameters.md) and its CLI view must be qualified before sweeping the
five parameterized originals. Both solvers must receive equivalent effective
constants, retained in the campaign identity. Do not edit source text or rewrite
terms in the runner to compensate for a missing solver API.

Preregister sizes 1 through 14 for the first series, including the UNSAT sizes
2 and 3. Use the same explicit eager/lazy × CPU/Metal configurations and clingo
reference as above, with separate first-answer and exhaustive-enumeration tasks.
In the first-answer series require a verified stable model, or exhaustive UNSAT;
different first models are permitted. In the exhaustive series compare complete
answer sets, displays and counts. Never carry the original `@count 92` or
`@expect sat` annotations into other sizes: those expectations describe N=8.
These are elenctic unit-test annotations, separate from ASP semantics. The solver
treats them only as comments. The separate corpus validator reuses them as external
expectations; they are not read or enforced by solver execution.
Preserve their original text and attach size-specific sweep expectations to the
experiment configuration instead of changing the comments.
Independently validate queen positions and attacks in each displayed witness.

Set per-invocation time, host/device memory accounting limits and output/storage
ceilings before execution, alongside repetition counts. Record every size and
configuration, including refusals and stopped runs. After a timeout or resource
stop, retain a prespecified rule for either continuing larger sizes or marking
them not attempted; do not retrospectively select a favorable stopping point.
Full enumeration has rapidly growing output, so preserve output/capture limits
separately from solver limits. A capture stop is not a solver scalability result.

Plot time and memory against N separately for the two semantic tasks. Include
admitted atoms, emitted instances, source joins, candidates, reduct work,
transfers and actual device work where measured. Report first-answer latency,
complete enumeration time and answers per second as distinct observations.
Comparing variants at the same N can expose dependence on encoding structure;
comparing N within a variant can expose scaling. Neither establishes a general
complexity bound from a finite empirical series.

## Execution and publication

The runner belongs to the Rust validation/experiment library; its installed
command composes typed configuration, bounded execution, comparison and report
views. Preserve raw captures, source/include and binary hashes, Git revision,
toolchains, macOS/driver/adapter identities, worker counts, device work and every
limit. Reports must not overwrite an input, executable or another evidence file
through lexical, symlink or hard-link aliasing.

Qualify the integrated build before timing. Pause competing builds, tests and
compute workloads, and record remaining host-load/thermal-control limitations.
Physical Metal runs must use an execution context exposing the M4 Pro adapter;
hosted CPU CI does not substitute for them. The README should link one complete
matrix report with all cases and dispositions once collected. Until then, retain
the narrower labels on existing performance results and mark this campaign pending.
