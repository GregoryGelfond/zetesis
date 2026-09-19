# Solver comparisons

Use the installed Rust `zetesis-perf` command in a quiet measurement window after
qualification. Sources, solver executables and report destinations are explicit:

```sh
zetesis-perf examples/kr-domains --suite baseline \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --report /new/path/baseline.json

zetesis-perf examples/kr-domains \
  --case scenarios/shortest-path/variant-01/01-basic.lp \
  --case standalone/send-money/send-money.lp \
  --memory-runs 5 --warmups 3 --repetitions 21 \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --report /new/path/selected.json
```

The named baseline contains SEND, queens02 and task allocation. `--suite queens`
contains all six unchanged eight-queens encodings. Repeated `--case` selects any
one through 94 distinct runnable cases from the sealed clean corpus manifest,
including optimized and inconsistent inputs. Original contracts survive as typed
manifest data; removed test annotations never become solver directives.

Ordinary comparisons use CPU, eager grounding, one closure worker, one completion
worker, the automatic reduct oracle and complete enumeration. Reference clingo
uses one worker and `optN`. Both receive unchanged source bytes, including the
verified transitive include closure, in a private directory. Every selected
contract and complete reported-display multiset must agree before a campaign
can pass. Symbol multiplicities, duplicate or empty displays, all optimum ties
and complete cost vectors remain significant. The decoder removes exactly the
supported `optN` final-incumbent discovery replay. Hidden atoms cannot be
reconstructed from `#show`.

Qualification precedes all warmups and timed pairs. Producer order alternates;
case order rotates between rounds. Ordinary timings cover direct process spawn,
parsing, grounding, solving, output capture and direct-child reaping. Hashing,
answer comparison and configuration queries are outside each timed interval.
Separate native `--stats` samples record phase attribution. There is no cache
flush or cold-cache claim. `--suite corpus` and explicit `--profile` matrices use
instrumented native JSON/statistics instead; their timing population is distinct.

Selected and memory campaigns retain bounded native version/short-help queries,
full help when advertised, and reference version/help queries. An advertised full
view must succeed; it cannot silently fall back to short help. Older help output
without that advertisement remains usable. Configuration queries consume the
same per-invocation and cumulative output allowances as solves. CLI reports seal
the runner executable as well as both solvers, the manifest, license and selected
source closure before execution, then recheck the seals afterward. Seals do not
attest dynamic libraries, hardware state or transient changes between checks.

## The fixed series

`--suite series` measures twenty-two fixed cells through the instrumented
matrix: generated family programs (independent sets, disjunction, tied optima,
closures, chains, stratified negation, a producer chain, Latin squares and a
planning line) at sizes whose complete native output fits 16 MiB, three amended
queens boards and two unchanged entries. Add `--profile cpu-auto` for the shipped defaults; the report retains
the grounding mode each cell took. `--time-limit SECONDS` gives every native
profile a cooperative deadline; `--oracle closure|countermodel` requests a
reduct procedure explicitly for every profile. The series raises the per-record, capture,
report and native-decoding byte ceilings to what its cells need (16 MiB per
record); raw reports are large and stay with their builds. Then derive the
retained comparison:

```sh
zetesis-perf examples/kr-domains --suite series --profile cpu-auto \
  --warmups 1 --repetitions 3 --timeout-seconds 30 --campaign-seconds 3600 \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --report /new/path/series-after.json

zetesis-series --report main=/path/series-main.json \
  --report before=/path/series-before.json --report after=/path/series-after.json \
  --json /new/path/series-comparison.json
```

The view prints Markdown tables of medians, ratios and counters and refuses
reports whose cells or profiles differ. Run the baseline again beside every
comparison: a change that also appears in the baseline column is drift, not
a gain.

## Separate memory observations

`--memory-runs N` appends zero through 41 paired resource observations per case.
It applies to ordinary CPU comparisons. Each starts a fresh Rust helper that
waits for exactly one solver, then reads `RUSAGE_CHILDREN.ru_maxrss`. macOS reports
bytes; Linux reports KiB, converted explicitly to bytes. The raw value/unit,
solver PID and exit, helper PID and exit, command, output and bounded resource
record remain inspectable. The helper itself is excluded. Usage propagated by
descendants reaped by the solver can contribute; this is **not** a simultaneous
process-tree RSS sum, GPU memory measurement or allocator accounting.

The parent owns the helper wait; the helper owns the solver wait. The solver
inherits the helper's process group. Deadline/output failures terminate that
owned group, as does a failed helper whose solver may have closed its pipes.
A trusted helper exits zero only after waiting for its solver and publishing its
separate resource/exit record. Neither that protocol nor ordinary capture proves
termination of escaped descendants. Memory durations never enter timed summaries.

## Bounded reports

Defaults are three warmups, 21 timed pairs, no memory pairs, 30 seconds per
invocation and 180 seconds for campaign scheduling. Warmups are bounded by five;
timed and resource populations by 41. `--timeout-seconds`, `--campaign-seconds`,
`--sample-bytes`, `--capture-bytes` and `--report-bytes` expose independent bounds.
Raw stdout/stderr defaults are 4 MiB per invocation and 128 MiB cumulatively.
Each private resource record is bounded by 4 KiB, with at most one additional
observed byte retained to demonstrate an exceeded bound. These ceilings do not
represent actual heap capacity, OS pipe storage or RSS.

Failed, malformed, incomplete or mismatched observations retain evidence and
stop subsequent launches; samples are never replaced. Report publication is a
separate bounded, no-clobber operation. A failed campaign receives no accepted
statistical summary. Successful schema-2 reports expose separate wall-nanosecond
and child-RSS-byte distributions with exact inclusive quartiles represented as
`whole + quarters / 4`; raw integer observations remain authoritative.

Preset ordinary campaigns without selection or memory extensions retain their
schema-1 schedule and timing protocol. Explicit selection or memory uses schema
2. Matrix reports have their own protocol/schema. Do not pool measurements from
different protocols merely because the input is the same.

The [validation library guide](../crates/zetesis-validation/README.md) describes
the reusable APIs. Recorded-report fixtures, synthetic executable fixtures and
process lifecycle tests establish comparison-tool contracts; they establish no
solver performance result.
