# zetesis command

Run one or more original ASP source files, or pipe source to standard input:

```sh
zetesis input.lp
zetesis input.lp --models 0
zetesis input.lp --models 0 --stats
zetesis encoding.lp instance.lp --models 0
printf 'a :- not b. b :- not a.\n' | zetesis --models 0
zetesis devices
zetesis --help
zetesis --version
```

Supported language constructs are available automatically. Every returned answer
set must satisfy the original program and the exact reduct acceptance criterion.
For example, `a :- a.` has only the empty answer set: `{a}` lacks reduct support.

Without objectives, the default stops after one answer set and `--models 0`
requests exhaustive enumeration. With an active objective, search always runs
to exhaustion to establish optimality, then displays up to the requested number
of tied optimal models. `--models 0` displays every full optimal model. Display
selection preserves hidden differences, so identical answer lines can represent
distinct answer sets.

`SATISFIABLE`, `UNSATISFIABLE`, or `OPTIMUM FOUND` and the separate coverage record
describe the result. A search or objective resource stop prints `INCOMPLETE` and
cannot establish UNSAT or optimality. Retained incumbents may still be displayed
with their fully evaluated costs. Exit 0 means the request completed, 2 means an
input/backend/output error, and 3 means search was interrupted. These are not
clingo's numeric exit codes.

`--stats` adds version, configured resource ceilings, requested execution policies,
completion, available counters and total driver wall time to stderr. It leaves
stdout answer records unchanged and is off by default. Formula execution reports
its actual single search worker, independently of the configured closure pool.
Closure work is marked unavailable because the driver does not accumulate it.
Automatic closure routing can change between batches; its effective device and
grounder may be untracked or mixed, while the existing backend diagnostics retain
actual adapter and fallback information. Unavailable counters are never printed
as zero.

An interrupted native search returns a partial `Report`, including retained batch
accounting when available. The additive `run_detailed`,
`run_detailed_with_diagnostics` and `run_bundle_detailed_with_diagnostics` APIs
retain a `RunFailure` on source, device, protocol, observation or output failure.
Its original typed cause is accompanied by an optional `PartialReport` and
attempted phase timings. Early source/setup failures can have no semantic report.
The existing `run` and diagnostic convenience APIs retain their
`Result<Report, RunError>` signatures and return the original cause.

Failure evidence separates verified stable models, pending batch candidates,
verified queued models and completely published Answer records. Closure verification
includes all accepted members of a completed batch, while its `checked` counter
retains the existing count of results consumed by the driver. A cost-bearing
Answer is counted only after its cost line also succeeds. An output sink can
accept a partial record, but that record is excluded from `published_models`.
`completion: None` means no search stopping classification was established;
`Some(Exhausted)` can coexist with a failed later publication. The separate
`summary_published` flag records whether the entire final coverage/count summary
reached the sink. Publication is neither a flush nor a durable-storage guarantee.
An unrelated checker/diagnostic failure does not flush retained incumbents or
emit a new completion summary. The partial report retains incumbent metadata and a previously observed logical
stop even if a later buffered Answer fails. Successful `Report` values retain
their existing interruption/coverage invariant.

The process adapter uses the detailed API. Failed runs with `--stats` print the
available partial accounting without claiming successful request completion.
If writing those statistics also fails, `secondary_output` retains that error
without replacing the original cause. This is terminal evidence, not a resumable
search checkpoint: pending interpretations, queues and device resources are not
returned to the caller.

The measured driver interval includes semantic admission, search and answer
output, and excludes original file/stdin loading and the statistics write itself.
It is not a device, kernel or CPU-time measurement. Loaded source
refusals and interrupted driver calls receive failed/incomplete statistics;
process-level failures before admission (such as unreadable files) have no driver
measurement. A failed statistics sink remains an output error, even if completed
answer records were already written. Neither partial coverage nor a retained
incumbent is labeled an established optimum.

The appended `Phase timings` section reports integer host nanoseconds and attempted
call counts. It separates admission/materialization, execution setup, initial
candidate setup, candidate generation/blocking, original-formula validation,
GPU host oracle calls, exact CPU reduct membership, CPU closure membership,
objective scoring/retention, objective feedback and observation/output. Setup
includes device initialization or an owned worker pool; a GPU host oracle call
includes transport, submission, waits and readback. It is never labeled kernel
time. Formula execution uses original validation and exact reduct membership;
closure execution uses closure membership instead. `unmeasured` means a phase
was not entered or does not apply; it does not assert zero work.

Intervals do not nest, but phase sums need not equal the driver total: source
retry preparation, formula interpretation-to-Model conversion and other
orchestration/timer overhead are unattributed. Admission includes materialization rather than separating parsing
from grounding; lazy closure membership includes source joins. Objective feedback
includes candidate-only bound compilation/restriction, not original reduct edits.
Failed returned attempts contribute elapsed time independently of the semantic
counters available in the detailed failure. A timing overflow reports `complete=false` and
retains only the measured prefix, without changing solving. Disabled statistics
read no clocks and make no timing-specific heap allocations; coarse optional
branches and fixed-size fields remain. Timing calls do not count models, queries
or semantic work. In particular a failed reduct encoding may consume a timed
attempt before the query counter advances. Public successful reports expose
`PhaseTimings`; detailed errors retain them even when diagnostics cannot be written.

## Reduct oracle selection

`--oracle auto` is the default. The solver first attempts the normal-rule closure
specialization, including scalar expansion and display metadata. If the source
needs richer supported constructs, it selects finite Ferraris reduct countermodel
checking. Syntax, undefined arithmetic, overflow, core admission and resource
failures are preserved; they never trigger a retry with a weaker limit.

Advanced controls make the exact procedure explicit:

```sh
zetesis input.lp --oracle closure --models 0
zetesis input.lp --oracle countermodel --models 0
```

`closure` checks sparse gate candidates by reduct least closure, constraints and
candidate agreement. `countermodel` checks complete semantic candidates and
rejects any candidate with a proper subset satisfying its frozen Ferraris reduct.
The latter materializes a bounded formula theory. Automatic and CPU selection
use native CPU search; an explicit GPU selects batched Ferraris propagation with
exact native CPU completion of residual queries. A required but unavailable
device is refused. Explicit lazy formula grounding remains unsupported.

The internal countermodel search uses a native chronological DPLL implementation
and full Tseitin equivalences. Those Boolean queries are replaceable implementation
machinery. Classical satisfaction alone cannot establish an answer set. Only
semantic atoms participate in minimality and model blocking; auxiliary variables
do not define additional answer sets.

`--max-search-work` and `--max-search-decisions` accumulate across encoding and
countermodel queries. `--max-candidates` bounds candidates, and `--max-work`
bounds each independent witness check. Formula admission caps atoms at 65,536,
nodes at 1,048,576 and roots at 262,144; smaller CLI atom/root limits also apply.
The internal encoding caps variables at 1,048,576, clauses at 4,194,304 and literal
occurrences at 12,582,912. Formula candidate search remains serial. By default
CPU formula execution uses the existing scalar cursor (`--completion-workers 1`).
Set `--completion-workers 2` or `4` to request a reusable Rayon pool for independent
exact reduct queries in ordinary CPU batches. GPU residual batches use the same
bounded executor, including when its requested worker count is one. This setting
is separate from closure's `--workers` and is never automatically increased.
Batched formula execution uses `--batch-size` and `--max-batch-bytes` to bound
proposals and transport. `--max-completion-scratch-bytes` defaults to 268435456
(256 MiB) and independently admits all batch result slots plus simultaneous query
workspaces before allocation. A tight budget reduces admitted query concurrency;
failing to fit one required query returns explicit incomplete coverage. This
logical budget includes reserved unused query slots but excludes allocator and
hash-table overhead, pool/thread storage, the shared theory, the scalar candidate
cursor and GPU memory. It is not a process memory limit. The direct CPU scalar
cursor does not use this completion-batch budget; `--stats` says so explicitly. Proposals
remain accounted for until a whole membership batch commits; verified models
awaiting objective scoring or output are retained separately. Formula search
reports its own counters, with gate-tuple discovery marked inapplicable.

With `--stats`, hybrid execution reports the actual adapter, GPU batches and
candidate worlds, charged propagation work and sweeps, CPU residual completions,
GPU decisions, pending candidates, queued verified models and peak authored
device-allocation accounting. CPU and hybrid batched execution also report
requested and peak admitted completion concurrency, peak logical scratch,
entered/completed/failed residuals, and pending/queued models. Completed local
checks remain uncommitted if any batch member fails. With timing enabled,
coordinator wall intervals are separate from summed worker intervals; overlapping
worker intervals are not added to scalar phase timings. Diagnostic overflow is
explicit and cannot discard committed answers. These are neither kernel timings
nor process RSS. Portable tests establish correctness, not a parallel speedup.
Device selection and initialization happen during ordinary invocation; no setup
script, qualification command or stored pass marker is required.

## Backend and grounder selection

`--backend auto` and `--grounder auto` are the defaults. On the closure path,
the empty seed is checked with lazy CPU joins. Later batches of at least 32
candidates may initialize a physical GPU and compile a static program. This is
a provisional scheduling threshold, not a measured performance crossover.

An automatic GPU admission or submission failure reports a reason on stderr and
retries unreported work on CPU. Explicit hardware requests never fall back:

```sh
zetesis input.lp --backend cpu
zetesis input.lp --backend metal --grounder eager
zetesis input.lp --backend vulkan
zetesis input.lp --backend dx12
zetesis input.lp --backend gl
zetesis input.lp --backend nvidia
zetesis input.lp --backend gpu
```

GPU support is included in the default build. `gpu` selects a physical adapter
through a compiled wgpu API; a named API requires that API. `nvidia` adds a vendor
filter through a graphics API, with no CUDA backend. Software, virtual and unknown
adapter categories are refused. `zetesis devices` lists compiled APIs and visible
capabilities without reading source. Actual device/shader initialization may
still fail. A CPU-only build uses `--no-default-features` at installation time.

For the closure specialization:

| Grounder | CPU | Automatic hardware | Explicit GPU |
|---|---|---|---|
| `auto` | Lazy source joins | Lazy CPU first; possible later static GPU batches | Static lowering |
| `lazy` | Source joins without a complete ground-rule store | CPU throughout | Refused |
| `eager` | Packed static closure scans | Retains the same static graph across GPU attempts and CPU fallback | Static lowering |

Static lowering is bounded by `--max-atoms`, `--max-substitutions` and
`--max-ground-rules`. GPU closure currently supports at most 4,096 atoms. Lazy
CPU checking instead uses `--max-work`, `--max-atoms` and candidate/carrier bounds.
Its charged operations differ from eager scans. General formula execution uses
eager admission on both CPU and explicit GPUs. `--backend auto` keeps the CPU
formula route pending a measured scheduling crossover. Lazy GPU templates,
GPU candidate search, exact GPU residual search and GPU objective scoring remain
future work; the current formula GPU route is explicitly hybrid.

## Source and objectives

Source admission includes finite safe normal and unconditional disjunctive rules,
constraints, finite conditional choices and bounds, evaluated positive heads and
top-level numeric/dependent intervals, default/double negation,
scalar constants/arithmetic and finite interval bindings, plus finite
count/sum/sum+/numeric min/max comparisons and assignments, bounded term and
conditional `#show` in addition to signature/empty selections, and `#defined`.
The [frontend profile](../zetesis-themelios/README.md)
specifies the exact safety, numeric and syntax boundaries. Recursive eligibility
remains in the reduct formulas; choice bounds provide no support. Choice-head
intervals retain one group; disjunctive intervals expand whole rule instances,
each preserving the original disjunction.

Formula construction uses a bounded possible-positive relation, then joins
against that relation to emit potentially relevant instances. This reduces
materialization but still constructs a complete finite formula theory before
search. It is distinct from the lazy closure oracle's candidate-specific joins.
`--max-substitutions`, `--max-expansion-work`, and formula atom/root ceilings
apply; the source value domain is capped at 1,024 and support closure at 1,024
rounds, including a final round establishing no changes.

The current `#minimize`, `#maximize` and positive weak-constraint profile accepts scalar
weights/tuples, constant numeric priorities, positive ordinary conditions and
scalar equality/inequality filters. Dependencies relevant to an objective have
additional explicit restrictions, including default-negated and disjunctive
producers and non-total aggregate observers. Unrelated rules and negative
constraints retain their ordinary semantics. These checks preserve observable
objective presence and priority slots. Classical sign is preserved in these
conditions independently of default negation. Compound logical terms,
richer weak bodies and broader directives remain implementation
work. Undefined/overflowing arithmetic is a located refusal.

Term observations run over verified full models and may construct output-only
functions and tuples. Ordinary positive atoms must bind their named variables;
supported negative and scalar comparison conditions filter those bindings.
Shown constructors create no logical atoms or support. Term-only directives
retain default atom output; signature/empty directives select the atom channel.
Equal enabled terms coalesce globally within the term channel, while an atom and
a shown term with the same spelling both appear. Hidden full models remain
distinct answers. Arithmetic/generative term displays and broader conditions
remain explicit refusals.

This observation source profile uses eager formula execution on CPU or an
explicitly selected GPU, with observations evaluated on the host after stability
checking. Explicit `--oracle closure` and `--grounder lazy` remain refused.
`--max-observation-work`, `--max-observation-bindings`, and
`--max-observation-terms` bound each displayed model's observation work;
`--max-observation-bytes` bounds retained term payload and the complete Answer
record independently. Internal observation failures emit no partial Answer.
Inputs with no term observations retain their existing output path.

Objective tuples are joined against each verified stable model, with global
(priority, normalized weight, tuple) deduplication and signed costs at descending
priorities. Maximization negates weights before forming those keys; an enabled
maximizing weight 2 prints cost -2. Equal keys coalesce across minimize, maximize
and weak statements. Eligible unrepresentable negation receives a located refusal.
They never supply atom support. `--max-objective-work` is cumulative across the
run; binding, key and key-byte bounds apply per model. Best tied models are kept
in a bounded store before display: `--max-optimal-models`, `--max-optimal-atoms`
and `--max-optimal-bytes` count full models before `#show`. Byte limits account
for canonical payload, excluding allocator overhead. A storage stop may preserve
an earlier valid incumbent even after a better score could not be retained;
coverage remains incomplete and optimality is never reported.

File arguments are ordered roots of one original source bundle, with global
constants and retained source identities. String `#include` lookup uses a working
directory captured once for the whole bundle, then falls back to the including
source's directory after a relative filesystem lookup failure. No source text is
concatenated. Exact repeated selected paths are included once; different lexical
aliases of one canonical source and include symlink redirections are refused.

`--max-source-roots` defaults to 256 root occurrences, `--max-source-files` to
256 unique files, `--max-total-source-bytes` to 8 MiB across those files, and
`--max-include-depth` to 32. `--max-source-bytes` limits each original file,
including children. Expansion and semantic limits cover the combined program.
Cycles and library includes are refused. Standard input (`-`) must be the only
root; repeated or mixed stdin arguments fail before reading any input. Stdin and
string library entry points have no implicit include base path. Explicit bundle
callers use `SourceBundle::load_many` and `run_bundle_with_diagnostics`.

`zetesis devices` is the command-first inventory form. Once a solver argument
or an input file has appeared, later `devices` arguments are file names. Use
`zetesis -- devices` to solve a first input with that reserved spelling.

`--max-expansion-work`, `--max-expanded-templates` and `--max-expansion-values`
bound normalization separately from solver work. Original file/span diagnostics
remain attached to refusals. The lower-level `admit`, `admit_extended` and
`admit_formula` library contracts remain distinct for clients requiring a
specific source boundary; the CLI makes the supported language available without
feature-unlock flags.
