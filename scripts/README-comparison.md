# Solver comparisons

Use the installed `zetesis-bench run` command in a quiet measurement window after
qualification. Sources, solver executables and report destinations are explicit:

```sh
zetesis-bench run examples/correctness --suite baseline \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --report /new/path/baseline.json

zetesis-bench run examples/correctness \
  --case scenarios/shortest-path/variant-01/01-basic.lp \
  --case standalone/send-money/send-money.lp \
  --memory-runs 5 --warmups 3 --repetitions 21 \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --report /new/path/selected.json
```

The named baseline contains SEND, queens02 and task allocation. `--suite queens`
contains all six unchanged eight-queens encodings. Repeated `--case` selects any
one through 94 distinct cases of the suite, here the whole sealed clean corpus,
including optimized and inconsistent inputs. Original contracts survive as typed
manifest data; removed test annotations never become solver directives.

Native runs use `solve --all --json --stats` with the requested profile, by
default the CPU, automatic grounding, the automatic reduct oracle, one
completion worker and the host's available parallelism for closure and candidate
search. An explicit `--threads N` sets that parallelism budget. Reference clingo uses one
worker and `optN`. Both receive unchanged source bytes, including the verified
transitive include closure. Every selected contract and complete reported-display
multiset must agree before a campaign can pass. Symbol multiplicities, duplicate
or empty displays, all optimum ties and complete cost vectors remain
significant. The decoder removes exactly the supported `optN` final-incumbent
discovery replay. Hidden atoms cannot be reconstructed from `#show`. Without
clingo, each native family is qualified against the workload's recorded
contract instead, and a workload without one is recorded as needing clingo.

Qualification precedes all warmups and timed rounds. Later rounds rotate both
case and producer positions. Timings cover process launch, parsing, grounding,
solving, instrumented machine output, capture and direct-child reaping; hashing,
answer comparison and configuration queries are outside each timed interval.
There is no cache flush or cold-cache claim. The ordinary comparison of
uninstrumented runs, whose separate `--stats` samples recorded phase
attribution, is a library protocol with no command (`performance::run`); its
timing population is distinct.

Each run retains bounded native version and full-help queries (`solve --help-all`,
or `--help-all` through the legacy interface), and the reference version query
when clingo takes part. A failed query leaves every planned position
unattempted, retained as such; nothing falls back to short help. Configuration queries consume the same
per-invocation and cumulative output allowances as solves. Reports seal both
solvers, the memory rounds' helper, the manifest, license and selected source
closure before execution, then recheck the seals afterward. Seals do not attest
dynamic libraries, hardware state or transient changes between checks.

## The fixed series

`--suite series` measures twenty-two fixed cells through the instrumented
matrix: generated family programs (independent sets, disjunction, tied optima,
closures, chains, stratified negation, a producer chain, Latin squares and a
planning line) at sizes whose complete native output fits 16 MiB, three amended
queens boards and two unchanged entries. The default profile requests the
shipped defaults; the report retains the grounding mode each cell took.
`--time-limit SECONDS` gives every native profile a cooperative deadline;
`--oracle closure|countermodel` requests a reduct procedure explicitly for every
profile. The series raises the per-record, capture, report and native-decoding
byte ceilings to what its cells need (16 MiB per record); raw reports are large
and stay with their builds. The amended cells have no recorded contract, so
they need clingo. Then derive the retained comparison:

```sh
zetesis-bench run examples/correctness --suite series \
  --warmups 1 --repetitions 3 --timeout-seconds 30 --campaign-seconds 3600 \
  --zetesis /path/to/zetesis --clingo /path/to/clingo \
  --report /new/path/series-after.json

zetesis-bench compare main=/path/series-main.json \
  before=/path/series-before.json after=/path/series-after.json \
  --markdown --output /new/path/series-comparison.json
```

The Markdown view prints tables of medians, ratios and counters, and a
scoreboard against the reference solver per report and profile, described in
the validation crate's README; it refuses reports whose cells or profiles
differ, the search method excepted under its present and its earlier
spellings, which that README names. Run the baseline again beside every
comparison: a change that also appears in the baseline column is drift, not
a gain.

## Separate memory observations

`--memory-runs N` appends zero through 41 paired resource observations per case,
after the timed rounds. Each starts a fresh Rust helper, `zetesis-bench` itself,
that waits for exactly one solver, then reads `RUSAGE_CHILDREN.ru_maxrss`. macOS
reports bytes; Linux reports KiB, converted explicitly to bytes. The raw
value/unit, solver PID and exit, helper PID and exit, command, output and
bounded resource record remain inspectable. The helper itself is excluded.
Usage propagated by descendants reaped by the solver can contribute; this is
**not** a simultaneous process-tree RSS sum, GPU memory measurement or allocator
accounting.

The parent owns the helper wait; the helper owns the solver wait. The solver
inherits the helper's process group. Deadline/output failures terminate that
owned group, as does a failed helper whose solver may have closed its pipes.
A trusted helper exits zero only after waiting for its solver and publishing its
separate resource/exit record. Neither that protocol nor ordinary capture proves
termination of escaped descendants. Memory durations never enter timed summaries.

## Bounded reports

Defaults are one warmup, three timed rounds, one memory round, 10 seconds per
invocation and 180 seconds for campaign scheduling. Warmups are bounded by five;
timed and resource populations by 41. `--timeout-seconds`, `--campaign-seconds`,
`--sample-bytes`, `--native-report-bytes`, `--capture-bytes` and `--report-bytes`
expose independent bounds. Raw stdout/stderr defaults are 16 MiB per invocation
and 512 MiB cumulatively; the native decoder reads at most 16 MiB and the
evidence at most 1 GiB. Each private resource record is bounded by 4 KiB, with
at most one additional observed byte retained to demonstrate an exceeded bound.
These ceilings do not represent actual heap capacity, OS pipe storage or RSS.

Failed, malformed, incomplete or mismatched observations retain evidence and
stop subsequent launches for that cell; samples are never replaced. Report
publication is a separate bounded, no-clobber operation. A timing distribution
exists only for a cell whose entire timed population passed; raw integer
observations remain authoritative.

Matrix reports have their own protocol and schema, distinct from the ordinary
library campaign's schema-1 and schema-2 reports. Do not pool measurements from
different protocols merely because the input is the same.

The [validation library guide](../crates/zetesis-validation/README.md) describes
the reusable APIs. Recorded-report fixtures, synthetic executable fixtures and
process lifecycle tests establish comparison-tool contracts; they establish no
solver performance result.
