# Region-worker engagement: start the workers once the walk outlasts their start

A parallel region walk used to start all of its worker threads at the first proposal, whatever the walk turned out to
need. This note makes the calling thread walk first, as the scalar walk does, and starts the workers only once the
walk has run for as long as starting them costs. It is an execution choice: answer families and admission are
unchanged at every worker count and at every engagement point. The [parallel execution
chapter](../book/rust/parallel.md) describes the walk.

Paths are under `crates/zetesis-sat/src/ferraris/` unless another root is named. Measurement cells under
`scenarios/` and `standalone/` are in `examples/correctness/`, and those under `scalability/` in `examples/`.

## Status

| Part | Status |
|---|---|
| The coordinator-first walk, its record, the engagement operation, the retained coordinator state, the public API, the CLI's record, documentation and tests | Implemented |
| The provisional engagement constant, 20 µs per worker, with its record | Implemented; M1 replaces the value |
| Revisions to statements and tests the coordinator phase affects | Partly implemented; remaining sweep in Open review items |
| The measurement protocol M1–M6 and the acceptance (M5) | Open |

Answer identity has been checked on 108 programs (the 94 corpus cells, the scalability programs and the top-level
examples): every outcome (status, completion, coverage, published models, optimum and ties) and every family of models
is identical to the walk before the change, with the default delay at 1, 2 and 14 workers and with the delay forced to
zero and to never at 14, 540 comparisons in all. At one worker the model sequence is the previous one and repeats
between runs. Answer families are compared after decoding the JSON stream's interned atoms to names; an optimization's
schedule receipts vary between runs at many workers before and after the change and are not compared.

## Problem

`ParallelRegions` (`parallel_regions.rs`), the formula route's CPU region search for more than one worker and the
default for `--threads N` with N > 1, started its `workers` threads at the first proposal. A walk that ends before its
workers have anything to steal pays for their start, their idle spinning and wakes, and the coordinator's joins, and
gains nothing.

Measured with hyperfine on `scenarios/shortest-path/variant-01/01-basic.lp` (the attribution case) with `--all --backend cpu --json --stats`, 300 runs,
on a quiet host:

| Search, workers | Wall (mean ± σ) | System |
|---|---|---|
| regions, 1 (the scalar walk, no thread) | 3.266 ± 0.142 ms | 0.969 ms |
| regions, 14 | 3.544 ± 0.169 ms | 1.647 ms |
| clauses, 1 | 3.206 ± 0.153 ms | 0.942 ms |
| clauses, 14 | 3.227 ± 0.162 ms | 0.954 ms |

Fourteen region workers add 0.278 ms of wall time and 0.678 ms of system time to this walk, about 20 µs of wall and
48 µs of system per thread started; the clause search, which starts no thread here, shows no such cost. A scaling run
of 200 runs gives wall time above the scalar walk of 0.357, 0.065, 0.199 and 0.214 ms at 2, 4, 8 and 14 workers. The
two-worker point, about fifteen standard errors above the trend, is unexplained: it is either drift between
hyperfine's sequential batches or a cost of the two-worker walk itself (M2). These figures difference the eager walk
against the scalar walk, so they include the parallel structure's own cost; the engagement cost proper is the eager
walk against an unengaged walk of the same build (M1), and the structure's cost is the residual (M3).

Before the change this cost moved small fourteen-worker cases behind clingo: at high resolution zetesis was faster
than clingo 5.8.2 on 46 of 94 corpus cells at fourteen workers against 67 at one worker.

How many walks end early is the question the benefit rests on, so it was measured before implementation (M0) with the
scalar walk's phase timings on all 94 corpus cells. The walk alone takes 80 µs at the median (quartiles 51 and 166 µs);
with the caller's time between pulls, 188 µs (130 and 362 µs). At fourteen workers, where the delay is about 280 µs,
the walk ends within it on 76 of 94 cells, and within it counting the caller's time on 65. The 18 beyond are the six
n-queens variants (1.3–46 ms), task-allocation's larger mix (14.9 ms), send-money (3.0 ms), one task-allocation cell
(0.73 ms) and nine graph cells at 0.29–0.63 ms. At two workers, where the delay is about 40 µs, 13 walks end within it
on the walk alone and none with the caller's time, so the rule engages as before and saves little there.

## Design

### The walk's record, and what it owns

The walk's state has two parts. **The record** says where the walk ran and what it did there, owns no resource, and
is what the observation reports; it replaces the old `started` flag:

| Variant | Holds | Entered |
|---|---|---|
| `NotBegun` | nothing | at construction |
| `Unengaged { since, deadline, elapsed, walking }` | the walk's beginning; `since + D`, or `None` for a walk that never engages; the time to the end of the last stepping pull's coordinator part; the coordinator's time inside stepping pulls | at the first pull |
| `Engaged { after, walking, launched, spawn }` | when the workers started; the coordinator's walking time before then; the threads actually spawned; the spawn loop's wall duration | by the engagement operation |

**What the walk owns** is held beside it, present only while owned: the coordinator's retained cell, the channel's
receiver and sender, the worker handles and the deques. Between pulls, `NotBegun` holds no cell and no handle;
`Unengaged` holds no handle, and holds the cell exactly while the walk has neither ended, engaged nor been closed;
`Engaged` holds no cell. A pull changes the record only if it steps, so once the walk has ended, closed or engaged,
`statistics()` returns the same record on every later call.

**Configuration happens once, when the walk begins.** One `Arc::get_mut` synchronizes the budget and installs the
certificate and the timing flag, possible then because nothing yet holds a clone. Engagement only launches. A timing
enable made after the walk has begun is ignored for the whole walk, as a late enable already was.

| Operation | `NotBegun` | `Unengaged` | `Engaged` |
|---|---|---|---|
| `set_filter` | installs | refuses, `LateRegionFilter` | refuses, `LateRegionFilter` |
| `restrict`, `tighten` | synchronize the budget, then charge | charge the shared allowance | charge the shared allowance |
| `permits_determined` | decides | refuses, `LateCertificate` | refuses, `LateCertificate` |
| `propose` | begins the walk | runs the coordinator loop | runs the receive loop |

Each refusal and synchronization happens at the same pull as before, the first. **The teardown**, run by `stop`, by
drop and by every failure path, is one operation in every state: close the walk; release the cell, if owned, through
the contained drop (a panic there raises `WorkerPanicked`, and the first stop raised stays the one reported); drop the
receiver and sender; join whatever handles exist, merging each joined worker's receipts and releasing every deque's
payloads and capacity; report the recorded stop. Every step is a no-op when nothing is owned, so a second teardown
changes nothing.

### Beginning the walk

The first pull polls cancellation (on cancellation the walk stays `NotBegun`), installs the configuration, records
`since` and the deadline `since.checked_add(D)` (an overflow, as `After(Duration::MAX)` produces, gives `None`: the walk
never engages) and enters `Unengaged`. With `D > 0` it creates the coordinator's cell; with `D = 0` it creates none
and engages at once, before any step, which is the previous eager walk. The deadline is asked through a crate-internal
test parameter of `propose_with`; production passes the clock's `now ≥ deadline`, and the in-crate tests pass a
predicate that answers after chosen steps, so a test can place the deadline between two steps of one pull.

### The coordinator loop

In `Unengaged`, `propose` returns `None` if the walk is exhausted, then loops with these guards in order:

1. **Cancellation:** return it.
2. **A closed walk:** return the recorded stop if there is one, otherwise `Incomplete::ClosedEnumerator`. The receive
   loop gains the same rule; before, it returned `ClosedEnumerator` whenever its receiver was gone, so after `stop()` an
   engaged walk hid a recorded stop, against `propose`'s promise to return a raised stop on every later call.
3. **A resolved frontier:** end the pull's coordinator part, release the cell and the frontier storage, set `exhausted`
   and return `None`. The order matters: the cell's report holds the receipts of the leaves this pull stepped through.
   This guard precedes the deadline, so a walk resolved before its deadline ends without engaging.
4. **The deadline:** if it has passed, end the pull's coordinator part, engage, and continue in the receive loop.
5. **A step:** take the newest region of deque 0 and advance it through the cell under the pull's lease. On a split or
   a region resolved without a model, iterate; on a model, return it; on an error or a panic, exit as below.

**Ending a pull's coordinator part** settles and drops the pull's lease, merges the coordinator's certificate and
reduct receipts into the merged totals and takes them, updates the record if the pull stepped, and folds the shared
allowance's spent totals into the caller's counters. The merge is fallible (`CounterOverflow`), as the join's is.

**Exits.** A model ends the pull's part and returns. A step's error is raised as the shared stop first, so it is the
stop reported even if the merge then fails, and the teardown runs. A panic merges nothing: the panicking pull's
certificate and reduct receipts are discarded with the cell, as a panicked worker's report is, while its lease is
settled and a reservation in flight charges its whole grant as it unwinds. A failed merge raises `CounterOverflow`; at
a pull holding a model the model is returned and the stop follows on the next pull, so models arrive before the stop
as the module promises, and at any other exit the stop is returned at once, never `None`. A failed release returns
`WorkerPanicked`, never exhaustion.

**No pull steps a released cell.** The cell is released only when the walk ends at guard 3 (later pulls return at
`exhausted`), when it is closed (guard 2 returns), or at engagement (the record is no longer `Unengaged`). Between
iterations the outstanding count equals the length of deque 0, every other deque is empty, no worker exists and the
original sender is held; should guard 5 find no cell or an empty deque, the loop returns the internal-inconsistency
refusal `InvalidWitness` and never claims exhaustion.

**Termination.** Each iteration returns, engages or steps. A step splits a region on an open atom or resolves it, so
the regions form a finite tree (at most `2^(n+1) − 1` for n atoms).

### The per-region step

The worker loop and the coordinator loop drive one crate-private unit, `advance`: it steps one region through a
workspace under a lease, publishes the region's phase timings as the difference from the driver's baseline, and
resolves the region once when it is resolved. The drivers differ only in where regions come from (the worker's deque
or a steal; deque 0), where a model goes (the channel; the caller), how long a lease lives (until the worker steals,
waits or sends; one pull) and what an error does. "The coordinator steps as a worker steps" is a property of one
function, so a change to the per-region protocol is made once.

### Engagement

`engage_with` takes the thread spawn as a parameter; production passes the real spawn and the in-crate tests an
injected one. Its precondition is an `Unengaged` walk, neither ended nor closed, whose pull has ended its coordinator
part. It releases the cell (a failed release raises `WorkerPanicked`, tears down and returns, starting no thread),
reads the clock once, reserves `workers` handles, spawns the slots in order and, on success, drops the original sender
and enters `Engaged`. On a refused reservation or spawn it records `Engaged` with the threads actually launched before
any cleanup, raises `Allocation`, tears down (joining the launched subset and discarding the models it queued), folds
the joined subset's work into the caller's counters, and returns the stop. Slot 0's deque holds the coordinator's
remaining frontier, so worker 0 continues the depth-first walk and the others steal from it.

### The engagement delay

**Rule.** The workers start once the walk has run for the engagement delay `D` of wall time, measured from its
beginning, the first pull, after the enumeration's original index is built. The caller's time between pulls counts: it
is time no worker overlaps in the coordinator phase, and counting it bounds the overlap forgone by `D`. The deadline is
checked on entry to each pull and after each step that returned no model; a step is never interrupted, so engagement
can be delayed by a step already in progress or by the caller's time before the next pull. The clock does not
interrupt either operation.

**The objective is the invocation's wall time**, the latency a caller and the corpus campaign see; system time is a
co-benefit and is reported. In the rent-or-buy argument the rent is walking without workers and the buy price `B(w)` is
the wall time engaging `w` workers adds: their spawns, start-up, idle spinning and wakes, and the final joins.

**Estimator.** `D = w × c`, with `w` the configured workers, since engagement starts all of them, and `c` a documented
constant, `ENGAGEMENT_COST_PER_WORKER` in `parallel_regions.rs`, measured on the reference host. Its record is in the
[observation records](../book/reference/observations/README.md), "The region workers' engagement cost": provisionally
19.9 µs per worker at fourteen workers, a fit of 19.8 µs over 2–14 workers, so 20 µs and a delay of 280 µs at fourteen.
Measuring `B` at run time would mean starting a thread, which costs about what it measures, on every invocation.

**The cost model.** In an idealized rent-or-buy model, delaying engagement for `D` forgoes at most `D` of overlap,
and a walk that ends within `D` avoids the engagement cost `B`. The model assumes instantaneous deadline observation
and the same remaining work and execution rates after engagement. Its worst case is a walk that ends just after `D`:
the rule pays `D + B` where the better choice paid `min(D, B)`, a ratio of `1 + max(D/B, B/D)`, which is 2 at `D = B`
and at most 2.25 while `D/B` lies in [0.8, 1.25], the band M1 requires. The implementation observes deadlines only at
pull and step boundaries; independently scheduled runs can also do different checking work. This model therefore
motivates the delay but does not guarantee a bound on completion-time differences. M5 checks the invocation's measured
overhead. A model-dense small walk can be carried past `D` by its per-model work and output, then engages and pays `B`
for little; M4 reports that share.

### Parameter and public surface

- `Engagement` selects the deadline: `AfterStartCost` (the default, `D = w × c`), `Immediate` (`D = 0`, the previous
  behaviour) and `After(Duration)`; `After(Duration::MAX)` never engages, since its deadline overflows.
- `StableModels::with_region_workers_engaging(theory, workers, engagement, limits, cancellation)` takes it;
  `with_region_workers` keeps its signature and uses the default; `ParallelRegions::new` takes the engagement.
- It is not a `Limits` field, since a deadline is not a ceiling, and it has no CLI option: the solve crate uses the
  default. The constant is crate-private, and the public contract states it in prose.
- The region statistics gain a workers record (`regions.rs`): `engagement: RegionWalkEngagement` (`NotBegun`,
  `Unengaged`, `Engaged`, with `label()` returning the stable identifiers `not_begun`, `unengaged` and `engaged`) and
  `engagement_delay: Option<Duration>`, `None` for a delay that never engages. The CLI writes each state's keys through
  one record, with `engagement_delay_ns` `null` in JSON and `never` in the human line for such a delay.

### What the coordinator retains between pulls

A worker reuses its workspaces across regions; the coordinator reuses them across pulls. Its state is a self-cell, as
the scalar walk's retained filter is: the owner is a clone of the shared structure's `Arc`, and the dependent holds the
coordinator's workspaces (narrowing scratch, reduct-membership state, certificate workspace, region-filter checker,
retired knowledge), its report and its phase-timing baseline. The checker is prepared once, on the first region not
refuted by original narrowing, and retained across pulls, so the `RegionFilter` retention contract holds. The cell's
parts start empty, so creating it costs one small allocation, and nothing for a walk that engages at once. It is
released at the walk's end in the coordinator phase, at engagement and by the teardown, always through the contained
drop, so a checker whose teardown panics becomes `WorkerPanicked`, never exhaustion.

**Filter receipts.** On a filtered walk the coordinator's preparation is counted as every worker's is: a walk that never
engages prepares at most one checker (before, up to `workers`), and one that engages at most `workers + 1`, because the
coordinator's checker is released at engagement, not handed on. On the hybrid route each preparation builds a
constraint checker charged to the shared constraint allowance.

**Receipts.** Region counts, candidates and countermodel counts reach the live counters inside the step, as a worker's
do; phase timings after each step, as a difference from the baseline; certificate and reduct receipts are merged and
taken when each coordinator pull ends, including one that fails with a typed stop and the one that engages, and are
then reset. Each merge adds only that pull's work, and merged peak bytes take the larger value. The implementation
found that the merge dropped `ReductStatistics::peak_workspace_bytes`, so every parallel walk had reported zero; it is
now merged as a maximum.

## Correctness

The change schedules the existing region walk: every leaf is decided by the same certificate or reduct query, the
partition law is unchanged, and membership certificates take nothing from the schedule or from narrowing knowledge.
Before engagement the coordinator walks deque 0 depth-first, which is the scalar walk's order, so models arrive in the
scalar walk's order; at engagement the remaining frontier passes, intact, to worker 0's deque, and the walk continues
as the eager walk would from that frontier. No region is stepped twice or dropped: the frontier is either in deque 0 or
in the workers' deques, and the outstanding count is unchanged across the handoff. Answer families are therefore exact
at every worker count and engagement point. Failures stay failures: cancellation, exhausted resources, panics and
refused launches are returned as stops and never become exhaustion.

**What changes in output.** The region statistics gain the workers record. Until the workers start, models arrive in
the coordinator's depth-first order, which is the scalar walk's and still the schedule's. On a filtered walk the
coordinator's checker counts among the preparations. The coordinator's certificate and reduct receipts reach the
enumeration after each of its pulls, so a snapshot taken in the coordinator phase holds them. Under `Immediate` a first
pull that observes cancellation returns before launching any worker. The `Parallel regions` diagnostic line and the
`ParallelRegions` observation say that the workers start once the walk outlasts their start.

## Costs

One clock read per coordinator pull at its entry, beside the deadline test's reads between steps; `walking` adds each
stepping pull's duration from that read, and the record costs no further read. With `--stats`, per-step phase timing
adds its two clock reads and live publication per step in the coordinator phase, as the scalar walk pays two per pull.
The cell costs one small allocation. A walk that ends within `D` avoids the engagement cost `B(w)`; M5 measures the
net saving and the overhead of walks that engage against the eager walk.

## Tests

The implementation's tests cover the coordinator loop's guards and exits, the record's states, the teardown in every
state (including a stopped walk that never began releasing its frontier storage to zero capacity, and a refused launch
joining the started subset), the closed-walk rule, the mid-pull handoff (a positive-program certificate whose loops give
leaves refuted between the steps of one pull, and the tight certificate, whose merged checks are counted once across
the handoff), the receipts of an unengaged walk against the scalar walk's, a panicking pull's receipts and counters, the
retained checker and the filter's preparation counts, and the CLI's record in both output forms. The handoff's
deadline test answers once the pull has stepped, past a refuted leaf where the test asks for one, with more than one
region in deque 0, and asserts that it answered. The caller's counters are compared with the allowance's spent totals
after the workers have joined.

## Measurement and acceptance

### Builds

| Build | Source | Role |
|---|---|---|
| P | the implementation's parent | the eager walk, the baseline |
| I | the implementation | the default, `D = w × c` |
| I-imm | I with the default's constant set to zero | engaging at once, as `Immediate` |
| I-never | I with the default's constant set to `Duration::MAX` | never engaging, as `After(Duration::MAX)` |

I-imm and I-never are local measurement builds that change only that one constant; they are never merged and add no
flag, feature or workflow. Every build is a release build of the CLI with the front profile, each in its own target
directory, and each binary's SHA-256 is recorded.

### Protocol for every timed comparison

- **A quiet host:** load below 2 for the 120 s before; no build or other measurement running; `fseventsd` below 5% CPU;
  `caffeinate`.
- **The instrument.** M5, the acceptance, times the invocation's wall time with hyperfine,
  `solve CELL --all --backend cpu --threads w --json --stats`. M1, M2 and M3 measure effects of a few tens of
  microseconds that a whole invocation's start and exit noise buries, so each run is still a fresh process (the first
  thread creation is paid as the CLI pays it) but the measured quantity is `statistics.phase_timings.driver_elapsed_ns`
  from the same invocation, with wall time recorded beside it.
- **Blocks.** Eight block pairs in ABBA order, each block `R` runs (hyperfine `-N --warmup 20 --runs R` for M5, and a
  small runner that records the driver's time in the same order for M1–M3).
- **The statistic.** For block pair i, `d_i` is the mean of B's runs minus the mean of A's. The estimate is the mean of
  the eight `d_i`, with interval `Δ̂ ± t × s_d / √8` (Student, 7 degrees of freedom). The block is the unit, where drift
  lives; ABBA cancels linear drift within each quartet, and drift that remains widens the interval, so no comparison is
  discarded or re-run on its spread. The median of per-run differences is reported without a criterion.
- **The error budget.** The acceptance judges 42 comparisons: M5's 30 (15 cells at 2 and 14 workers), M1's 8 (two cells
  at four worker counts) and M3's 4 (two cells at two worker counts), under one familywise budget of 5% (Bonferroni):
  `α_c = 0.05/42 ≈ 0.00119`, one-sided for M3 and M5 with `t = 4.636`, two-sided for M1's band with `t = 5.247` on
  each side. A correct implementation, one whose criteria's premises hold, then fails by sampling alone with probability
  at most 5%.
- **Sizing `R`.** With `SE = √((σ_A² + σ_B²) / (8R))`, a criterion whose premise holds with margin m fails with
  probability `E[Φ(t·S − m/SE)]`, where `S = √(χ²₇/7)` is the interval's realized width in standard errors. The target
  is `SE ≤ m / δ*`, where `δ*` solves `E[Φ(t·S − δ*)] = α_c`: 9.517 one-sided and 10.940 for each side of M1's band,
  computed by numerical integration over the χ²₇ density. `R` is the smallest value of at least 20 that meets it.
- **The cap.** 2,500 runs per block. A comparison whose required `R` exceeds it is declared before any run as reported,
  not judged, and brought to the maintainers; `α_c`, `t` and the targets are then recomputed over the comparisons that
  remain.
- **The pilot**, before M1, M2, M3 and M5: for every comparison, the A arm in two blocks of `R_0` runs (200 on small
  cells, 50 on long ones); `σ_A` pooled, `σ_B = σ_A` for sizing; `R` computed and the declarations made; all of it
  recorded. Its runs are not reused.
- **Families.** Every run's answer family is checked equal across builds and to the scalar walk's.

For illustration, at the provisional figures and a driver-time spread of 0.05 ms, M1's band needs about 1,169 runs per
block at two workers and 24 at fourteen, and M3 about 566. At the whole-invocation spread of 0.15 ms, M1 at two and four
workers and M3 would exceed the cap, which is why M1–M3 read the driver's time.

### M1: the engagement cost and the estimator

On the attribution case and `scalability/prime-sieve.lp` (n = 30), A = I-never and B = I-imm at 2, 4,
8 and 14 workers, on the driver's time. The protocol's B-minus-A difference gives the positive engagement cost
`B̂(w)` with its interval per cell; each run's `spawn` receipt is recorded.
Per cell, a weighted least-squares fit through the origin gives `ĉ`; the documented `c` is the inverse-variance-weighted
mean of the two cells' `ĉ`; the fit `B(w) = a + w × c` is also made and its intercept reported with its interval,
without a criterion. **Criterion**, per cell and judged `w`: the two-sided interval of `B(w)` lies within
[0.8, 1.25] × `w × c`. That band is the practical test of an intercept: a fixed cost per engagement large enough to
matter moves `B(2)` out of its band first. The two cells' constants agree within 25%. If a worker count is declared
reported, the estimator is accepted on the judged counts provided 8 and 14 workers are among them on both cells. M1's
value replaces the provisional constant and its record.

### M2: the two-worker point

Reproduce it on the attribution case with P: A = four workers, B = two, on the driver's time, one-sided with margin
`B̂(2)/2`. If the interval's upper end lies below the margin, the earlier point was drift; record it and close. If the
lower end lies above it, separate engagement (I-imm − I-never at two workers) from structure (I-never at two workers −
I at one), and if engagement holds it, profile two workers against four (thread creation, the first spawn, the idle
wait's interplay) and record whether it is a real fixed cost or an explained exception. Otherwise M2 is inconclusive,
and M1's two-worker band is reported, not judged. M2 diagnoses rather than judges, so it is outside the familywise
budget, at the same per-comparison confidence.

### M3: the residual

I-never at 2 and 14 workers against I at one worker, on both M1 cells, on the driver's time. **Criterion:** the upper
end of the residual's one-sided interval is below `B̂(2)/2`, for a premise of at most `B̂(2)/4`. M3's arms run with
`--stats`, which charges the coordinator's per-step timing, so the timed residual bounds from above what an invocation
without `--stats` pays; a failure is re-measured without `--stats`, by wall time, and reported.

### M4: reach

With I-never at 2 and 14 workers, 50 runs per cell, record `Unengaged { elapsed, walking }` for every named cell and
classify each (cell, w): N, never engages (`elapsed < D(w)` on every run); E, engages (`walking ≥ D(w)`); C, the
caller's time decides (`walking < D(w) ≤ elapsed`); M, mixed. Per-step timing moves cells toward E and C, never toward
N. I's own records on the acceptance runs confirm the classes. The class C share is reported, not judged.

### M5: acceptance

**Cells**, families checked against the scalar walk. Small: `scenarios/shortest-path/variant-01/01-basic.lp` and its
task-allocation and traveling-salesman counterparts, `scenarios/equality-generalized-tsp/01-basic.lp`,
`scalability/pigeonhole.lp` (h = 5 and 7),
`scalability/subset-sum.lp` (n = 12) and `scalability/prime-sieve.lp` (n = 30), at 2 and 14 workers and at 1 for
reference. Long: `scalability/n-queens.lp` (n = 8, 9, 10), `scalability/mastermind.lp` (6 and 8 colours),
`standalone/n-queens/variant-02.lp` and `scenarios/task-allocation/variant-04/05-larger-mix.lp`, at 2 and 14 workers.

**Accept when**, for every cell at 2 and 14 workers, against P:

- **class N:** the lower bound of `wall(P) − wall(I)` is above zero and its estimate is at least half of `B̂(w)`;
- **classes E, C and M:** the upper bound of `wall(I) − wall(P)` is at most `D(w)` plus 1% of `wall(P)`;
- M1's criterion holds at every judged worker count on both cells, and the cells agree;
- M2 is explained, and M3's criterion holds;
- answer families are identical everywhere.

A comparison declared reported is given with its interval and brought to the maintainers. System time is reported per
cell without a criterion.

### M6 and the reports

**M6, the filtered route.** `standalone/n-queens/variant-01.lp` with `--grounder lazy --all`, which installs the region
filter, with P and I at 2 and 14 workers: the filter's preparations are at most 1 on runs that did not engage and at
most `w + 1` on runs that did; families are identical; constraint receipts and wall time are reported beside P's.

**Reports.** The high-resolution corpus campaign and the scoreboard, at 1 and 14 workers, with reach per class (walks
that never began, stayed unengaged and engaged) and margins against clingo, as a report. At the branch head, against
clingo 5.8.2 at fourteen workers, zetesis is faster on 73 of 94 corpus cells (69 clearly, 16 clear losses, geometric
mean ratio 0.874), against 59 (36 clearly, 22 clear losses, 0.944) at the checkpoint before this change; the head also
carries other changes, and the one-worker standing moved little (73 of 94 against 75).

**Records.** Every measurement keeps its commands, each build's source revision and SHA-256, the cases' SHA-256s, the
host's load at start and end, and the pilot's figures and declarations. M1's figures also go to the repository's
observation record.

## Alternatives

- **A thread-0 first phase,** worker 0 started at the first proposal and the rest at the deadline. It overlaps the
  caller's per-model work from the start, but every walk starts at least one thread, and it adds two concurrency
  transitions with their own termination argument.
- **One lifecycle value owning the coordinator's state.** The record had no home after the walk ended, and the teardown
  was stated per phase instead of per owned resource. Kept apart instead.
- **The coordinator continuing as worker 0 after engagement.** It saves one spawn but gives the coordinator two roles at
  once, with a termination condition spanning both.
- **Building the coordinator's checker per pull.** It breaks the `RegionFilter` retention contract and, on the hybrid
  route, repeats a charged preparation per model.
- **Holding the filter in its own `Arc`,** or **handing the coordinator's checker to worker 0.** The first keeps
  `Arc::get_mut` available, which nothing needs, at the cost of a second lifetime in the step; the second needs the
  checker's borrow re-anchored on the worker's clone, which means rebuilding it.
- **An `Engagement::Never` variant.** `After(Duration::MAX)` expresses it.
- **A run-time probe of `B`,** or **a process-wide cache of the measured cost.** The probe costs about one worker's
  start on every invocation; the cache couples walks process-wide, and is deferred.
- **Keying engagement on frontier width.** A depth-first split always leaves a sibling queued, so it starts threads on
  the early splits of small walks.
- **A fixed delay unrelated to the start cost, fewer workers by program size, or deferring the Rayon pools.** A
  constant without a cost argument; a heuristic about parallelism; no maintained invocation reaches the pools.
- **A significance test on M1's intercept.** It passes on imprecise data and fails on precise data; the band judged
  with intervals replaces it.
- **Whole-invocation timing for M1–M3.** Its noise needs about ten thousand runs per block for the two-worker band.
- **A deadline seam through the clock.** Every clock read would go through it, the record's included, and a test would
  have to count reads to place a step; the deadline test is asked only at guard 4.
- **Discarding and re-running drifted comparisons.** It would discard about half of drift-free comparisons; intervals
  built from block-pair differences replace it.

## Decisions

1. The per-worker engagement cost `c` is a documented constant with its measurement record in the repository, and the
   delay in force is reported with every walk.
2. The public API gains `Engagement` and `StableModels::with_region_workers_engaging`, and the region statistics gain
   the workers record.
3. The walk is coordinator-first: the calling thread walks until the deadline, then the workers start.
4. The caller's time between pulls counts toward the delay. The deadline is observed on a pull or between steps,
   so a step in progress or time before the next pull can postpone engagement. Completion-time overhead is measured by
   M5, not guaranteed by the delay alone. M4 reports the share of walks whose engagement the caller's time decides.
5. The corpus standing against clingo is a report, with reach and margins per class; M5's per-cell criteria decide
   acceptance.
6. If `D = w × c` fails its band, `D = a + w × c`, with both constants in one record, is adopted only for a fixed cost
   M2 attributes to engagement and M1's bands confirm at 8 and 14 workers; otherwise, or if the two cells disagree, the
   estimator is redesigned.
7. The record is named for engagement: `engagement: RegionWalkEngagement` with `NotBegun`, `Unengaged` and `Engaged`,
   and `engagement_delay: Option<Duration>`; `Engagement`, `RegionWalkEngagement`, `RegionWorkerStatistics` and
   `RegionSearchStatistics` are `#[non_exhaustive]`.

## Open review items

1. Revise the statements the coordinator phase made false, which still say the workers start at the first proposal or
   that only workers step regions: three in `ferraris.rs`, five in `parallel_regions.rs`, one in
   `crates/zetesis-sat/tests/integration/parallel_regions.rs` and one in `docs/book/rust/parallel.md`; then sweep the
   remaining worker-only statements.
2. Parameterize the route tests over `Immediate` and `After(Duration::MAX)` and assert the route each takes, including
   `NotBegun`.
3. Loop the copied retention tests over the scalar and coordinator routes, retaining each route's teardown contract.
4. Run M1–M6 and the reports in a quiet window, and replace the provisional constant and its record with M1's value.
