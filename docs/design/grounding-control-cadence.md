# Observe control at the metered cadence in grounding's work account

Grounding's work account observes cancellation and deadlines at a charged-unit cadence. This note states
the metered contract the CPU's metered operations already follow ([outcomes](../book/rust/outcomes.md), the metered
operations passage):

- control is observed before the first charged operation and at least once every `POLL_INTERVAL` (256) charged
  operations;
- a refused work charge observes control first;
- every operation settles its outcome at its exit, so a stop observed there takes precedence over every resource refusal.

Observation decides only when a run stops; it never changes what a completed run charges or answers.

## Status

- **Stage 1** (grounding's work account): implemented; correctness and performance acceptance remain separate gates.
- **Stage 2** (model construction, the observation encoders and the lazy oracle): a later stage, after Stage 1 is
  accepted, and accepted only on its own gain condition.
- Paths are under `crates/zetesis-themelios/src/` unless another crate is named.

## Problem

Grounding charges its work through one account, `Counters` (`formula_support.rs`). Every charge reaches the free
function `charge_work`, which observes control at every call: `Cancellation::poll` (`zetesis-cpu`
`cancellation.rs`) builds a `CancellationPoll` view by dereferencing the token's shared flag, deadline owner and slot
membership, and then reads up to three relaxed atomics. Most charges carry one unit of node- or edge-scale work (in a
fact-heavy probe, source evaluation averages about 18 ns per unit), so the observation is a measurable share of every
unit-sized charge. The CPU's metered operations already observe once per 256 charges (`metered_charge`, `Meter`).
Four other accounts predate that rule: model construction's `permit` (`zetesis-solve` `model_construction.rs`), the
observation encoders (`observation/view.rs` `Buffer::step`, `observation/evaluate/work.rs` `Work::step`), the lazy
oracle's `Work::charge` and `Work::tick` (`zetesis-cpu` `oracle.rs`), and the expansion budget's `charge`.

**Measured reach** (share of all samples inside a cancellation poll, by the nearest enclosing poller, release CPU
front at one worker):

| Profile | In a poll | Grounding account | Model construction | Lazy oracle and comparison callbacks | Encoders |
|---|---|---|---|---|---|
| sudoku | 3.5% | 2.9 | 0.3 | – | 0.0 |
| prime-sieve(300) | 3.7% | 2.7 | 0.9 | – | – |
| nested Mastermind | 3.0% | 2.1 | 0.5 | – | – |
| n-queens | 2.5% | 1.6 | 0.5 | – | 0.0 |
| shortest-path, variant 04 case 06 | 2.9% | 1.6 | 0.4 | 0.3 | 0.4 |
| traveling-salesman, variant 05 case 05 | 2.9% | 1.5 | 0.2 | 0.2 | 0.2 |
| fact files with range or comparison rules | 2.5–3.7% | 1.0–1.8 | 0.9–1.1 | 0.0 | 0.2–0.5 |
| shortest-path, variant 01 case 01 (tiny) | 1.7% | 0.9 | 0.1 | 0.0 | 0.1 |
| Mastermind | 3.1% | 0.1 | 2.5 | – | 0.1 |
| plain fact files (lazy route) | 3.5–4.8% | – | – | 2.5–3.6 | 0.7–0.8 |

The cost is present in every class profiled, between 1.7% and 4.8% of samples.

## Contract

Stated for the operations of grounding's work account, the ten scopes listed under Stage 1: formula preparation's
charged work, source classification and evaluation, instantiation, the terminal partition and admission,
constraint-checker preparation, each constraint check and region pass, and each answer's reconstruction.

**Preserved.** Answers, output and model order; the work charged and every receipt of a completed run (the cadence
changes when control is observed, not what is charged); every ceiling and failure kind; a stop or deadline still ends
the run as a typed interruption (`FormulaFailure::Interrupted`, `ReconstructionError::stop`,
`ConstraintCheckFailure::stop`), never as unsatisfiability, exhaustive enumeration or proved optimality; a failure keeps
its site and its `Program` envelope, including a failure a pending stop outranks.

**Changes.**
- **Where control is observed:** at the operation's entry; at every charge whose unit range contains a multiple of 256;
  at an explicit observation before work with no control hook; before refusing the work ceiling; at the exit before a
  completed result is returned; and at the exit when a resource refusal is settled.
- **Latency:** after a stop is requested at count `c`, at most 255 further units are admitted before an observation (the
  charge admitting the least multiple of 256 at or above `c` observes). Work with no control hook that is not bounded
  per element is preceded by an explicit observation. A finite table's allocation at one of its allocation boundaries
  is no longer preceded by an observation: under a pending stop it is admitted against the table's byte ceiling and
  made, and the stop is observed by the next boundary-crossing charge or at the exit.
- **Precedence:** each scope settles its outcome at its exit, so a resource refusal under a stop observed at that exit is reported
  as the stop, at the refusal's own site and inside its envelope. Today a stop takes precedence only over the work
  ceiling and over refusals some charge happened to precede, so this tightens precedence. It inverts one deliberate
  ordering: a zero allowance refused before any charge (the empty candidate's value ceiling) is reported as the stop
  when a stop is pending at the exit.
- **Completion:** the entry observes once more before returning a completed result and reports any stop observed there.
  A concurrent stop arriving after that observation requires the caller's next observation; this is not an atomic
  guarantee about the interval between the final poll and return.
- **Interrupted runs** can record up to 255 more units than today, all admitted before the observation.

## Stage 1: grounding's work account

**The boundary rule, shared.** `zetesis-cpu` exposes the range form of its private boundary test:

```rust
/// Whether admitting `amount` operations after `charged` admits one whose
/// zero-based number is a multiple of POLL_INTERVAL. A zero amount admits
/// nothing and is never due; the test computes no sum, so it cannot overflow.
pub const fn poll_due(charged: u64, amount: u64) -> bool {
    let offset = charged % POLL_INTERVAL;
    amount != 0 && (offset == 0 || POLL_INTERVAL - offset < amount)
}
```

`Meter::charge` keeps its form, the one-unit case, so both arrangements share one definition and one constant.

**Three operations of the account.**
- `Counters::charge_work(amount, limits, site)`: charge before perform; observes control when `poll_due` holds over the
  cumulative work already loaded for the ceiling check; on a refusal observes control first. Only its first statement
  changes on the hot path.
- `Counters::poll(site)`: an explicit observation that charges nothing, used before work with no control hook that is
  not bounded per element. No zero-unit charge is used as an observation any more.
- `Counters::absorb(work, limits, site)`: counts work a nested operation already performed under the allowance it was
  given; observes when `poll_due(work_before, work)` holds, so an absorbed range of 256 or more units always observes;
  an overrun is refused at the ceiling. This is the one departure from charge before perform, at the sites where it
  exists today.

**The finite table's zero requests.** `Table::prepare_with` and `Table::select_with` (`zetesis-cpu` `table.rs`) call
their caller's callback with zero at entry, at allocation boundaries and before each local work check, so today every
table tick observes control twice. Grounding's callback maps a zero request to no observation; latency over the table's
work is carried by `poll_due` on its positive charges, precedence by the entry's settlement, and a stop pending at the
start by the entry's observation. The callback contract's text changes from "zero polls control" to "zero marks an
observation point; the callback observes control there by its own rule (the legacy control observes at every one)".

**Work with no control hook.** The inventory, with what brackets each:

| Site | Work | Before | After |
|---|---|---|---|
| `formula_domains.rs` | `zetesis_domain::analyze` | `poll` (was a zero charge) | `absorb` |
| `formula_terminal/partition.rs` | `terminal::analyze` | `poll` | `absorb` |
| `formula_support/relations.rs` | `Catalog::new` over a predicate's rows | `poll` | `absorb` |
| `formula_support/queries/domains.rs` | `relation.query_attempt` | `poll` | `absorb` |
| `formula_support/relations.rs` | `append.insert`, `lookup`, `lookup_key` of one atom | per element | `absorb` |
| `formula_support/relations.rs` | a refused catalog operation's reported work | its own bracket | `absorb` |
| `formula_analysis.rs` | themelios `Analysis::of` | the expansion budget's poll | unchanged |

**One entry creates the control.** `Accounting::controlled` (`formula_support/accounting.rs`) becomes the one entry
that gives the account control: it observes at entry, lends the operation the only counters that carry control,
observes again before a completed result is returned, settles a resource refusal under a stop observed at exit as the stop
(at the refusal's own site, or the entry's when it has none, inside its envelope), and moves the account in and back
out on every outcome, unwinding included. Its error type is any `E: StopPrecedence`, a crate-private trait
(`is_resource_refusal`, `outranked_by(stop, site)`, `from_stop(stop, site, receipt)`) implemented for the four error
types the scopes return: `FormulaFailure`, `ReconstructionError`, `ConstraintCheckCause` and `ConstraintCheckFailure`.
Because `outranked_by` reads the replaced error, it keeps the site and envelope, which the public
`zetesis_cpu::Refusal::from_stop` cannot; the public trait does not change. The entry's own stops (at entry, or before
completion) take exactly the shapes today's observations produce at the points the entry replaces.

The token is held in `AccountControl`, defined in `formula_support::accounting` with no
`Default`, no `Clone` and no constructor visible outside it. Creating control is therefore possible only through the
entry (and a test-only writer for unit tests of inner operations), by type. Moving a whole controlled account out of
its scope (`mem::take` or `mem::replace` on the lent counters) stays possible inside a scope; escaping it is excluded by convention and checked in debug builds by a scope flag the entry clears
at its exit, which `AccountControl::observe` asserts open. The production formula builder now borrows the lent counters;
its success, refusal and unwind all leave the complete account with the entry. Publication keeps that borrow through
its last resource check. Terminal admission retires generated membership in place, retaining its account through the
final generated-value and storage ceilings before handing the completed baseline onward.

**The ten scopes.** Every production site that gives a `Counters` a token becomes a `controlled` scope:
`formula.rs` (`prepare`), `formula_source.rs` (`classify`, `evaluate`), `formula_ground.rs` (`instantiate`),
`formula_terminal/partition.rs`, `formula_terminal/materialize.rs` (closing the base and the terminal admission's tail
ceilings), `formula_hybrid.rs` (`checker_with_allowance`, `examine`), `formula_hybrid/consequences.rs` (each region
pass) and `formula_terminal/reconstruct.rs` (each answer). Theory validation stays outside: it charges no account, and
its resource refusals keep today's order.

**Settlement.** `is_resource_refusal` classifies every variant of `FormulaFailure` and every nested cause family by the
definition `zetesis_cpu::Refusal` states: work, storage, a named ceiling, a size the host cannot represent, or an
allocation. Shape, identity, capability and invariant errors are not resource refusals; a variant that already reports
a stop passes through. Each match is exhaustive, with no wildcard arm. The four families whose owners already classify
them are read through public methods published once by each owner: `zetesis_core::catalog::Error::is_resource_refusal`,
`zetesis_core::ModelError::is_resource_refusal`, `zetesis_ferraris::AdmissionError::is_resource_refusal` and
`zetesis_ferraris::AggregateErrorKind::is_resource_refusal`, with tests pinning that each agrees with its former users.

## Stage 2: the other accounts (later)

- **Model construction** becomes `Cancellation::metered` per preparation and selection, `permit` charging the meter.
- **The observation encoders** take the cadence on their own counts with `poll_due` and settle at their single exits,
  each the only constructor of its encoder's work.
- **The lazy oracle** (`zetesis_cpu::oracle::Work`) takes the cadence on its work count; a settled constructor,
  `Work::settled`, becomes the only production constructor of a `Work` (fourteen construction sites), so every
  operation settles by construction. A caller's evaluator or consumer error passes through unread.
- **The expansion budget** keeps observing at every charge (at most 0.2% of samples; a third arrangement is not worth
  it).

## Correctness

- **Semantics.** A completed run's result and receipts are identical: the charge path, amounts and ceilings are
  unchanged, and observations charge nothing.
- **Latency.** For charges with counts `c₀, c₁ = c₀ + a₁, …`, charge `i` observes when `poll_due(cᵢ₋₁, aᵢ)`. If a stop is
  requested at count `c` and `m` is the least multiple of 256 with `m ≥ c`, the charge admitting `m` observes and at most
  `m − c ≤ 255` units are admitted before it; if no charge admits `m`, the exit observes. Explicit observations only
  shorten this. Work with no control hook is counted when absorbed; unbounded work starts only after an explicit
  observation; per-element work adds one element's operation to the bound.
- **Precedence.** The work ceiling observes before it refuses; every other resource refusal of the ten scopes is settled
  at the entry's exit by the exhaustive classification; no scope can be entered except through the entry. The inverted
  zero-allowance ordering stays true inside the operation; at the exit a zero allowance is a named ceiling, and the
  documented rule is that a requested stop takes precedence over every resource refusal.
- **Completion.** The completion observation can replace a completed result by an interruption, never an interruption
  or failure by a result, and an interruption claims nothing about the program.

## Costs and accounting

- Removed per charge: the poll view's construction and up to three atomic loads; per table tick, the zero request's
  observation. Added per charge: one remainder, compare and branch; per 256 units, one observation; per operation, the
  entry's two observations; per unbounded interval, one explicit observation.
- **Expected effect** (grounding phase, the account's poll share over the grounding share): sudoku about −3.6%,
  prime-sieve(300) up to −3.9%, n-queens about −3.0%, nested Mastermind about −2.5%, fact files about −2.6%. Stage 2:
  Mastermind about −2.5% end to end; lazy-route fact files up to about −4%.
- Completed runs: every receipt unchanged. Interrupted runs: up to 255 more units recorded. No new uncharged work. No
  refused↔completed flip can arise from the cadence; only the classification under a pending stop changes.

## Tests

Seams, crate-internal and test-only: `Counters::attach_test_control` and `detach_test_control`, and
`observe_every_charge(|| …)`, a thread-local under which grounding's charge treats every nonzero amount as due, so the
existing interruption tests keep their propositions.

New tests: `poll_due` against its definition over all `charged < 1024`, `amount ≤ 1024` and near `u64::MAX`; a stop
before the entry is observed at entry, for each of the four error types, in today's shapes; a stop is observed by the
charge admitting the next multiple (requests at 1, 255, 256, 257 and inside a batch); a late stop is observed before the
result returns; a pending stop outranks the work ceiling; one settlement test per family (refusal reported as the stop
at its site, inside its envelope; a shape refusal of the family passes through); a zero charge neither observes nor
counts; an explicit observation finds a pending stop; unhooked work never starts under a pending stop; the table
observes at the account's cadence; a stop pending at a table allocation is reported at the exit; the terminal
admission settles its tail ceilings; a controlled account charged after its scope panics in debug builds; completed
receipts are unchanged; the published classifications agree with their former uses.

Existing tests that attach control to the account use test-only writers retaining their nonzero-charge observation
propositions; two that assert the zero-value ceiling precedes a pending stop are split into the inside-the-operation
proposition (unchanged) and the entry's settlement (the stop is reported).

## Documentation and Lean

- The outcomes chapter's metered-operations passage gains grounding's work account by operation name, the explicit
  observation, the absorbed charge and the entry's completion observation; theory validation is named as keeping its
  order. The grounding chapter's "zero work charges still poll cancellation" becomes "a zero charge admits nothing and
  observes nothing". `prepare_program_formula_with`'s documentation, `charge_work`'s, the entry's and the table
  callback's contracts follow.
- **Lean: `proofs/Zetesis/ChargeCadence.lean`.** `PollDue` describes range coverage for any positive period;
  `dueRule_iff` relates that proposition to the overflow-free distance rule used by `poll_due`. `pollDue_zero`,
  `fewer_than_interval`, `distance_reaches_boundary`, `distance_minimal`, `observes_covered_boundary` and
  `observes_next_multiple` state the zero, distance and charged-range laws. These are natural-number statements;
  atomics, scope coverage, error classification and concurrent flags remain Rust correspondence obligations.
- CHANGELOG, Compatibility: where the account's operations observe control, the new precedence, `poll_due`, the four
  published classifications and the restated table callback contract.

## Acceptance

On a quiet host (load below 2, fseventsd below 5% CPU, no build or other measurement), one worker, binaries A (the
parent) and B identified by commit and SHA-256:

- grounding phase: an interleaved A/B, 21 rounds, recording each run's exit status; end to end: hyperfine `-N`, 25 runs;
- answer and output identity over the corpus, scalability and example programs, and receipt identity (every
  non-timing statistics field of each completed run, A against B);
- Stage 1 passes if identity holds everywhere, if sudoku, prime-sieve(300), n-queens and nested Mastermind each fall in
  the grounding phase by at least 1% with separated ranges, and if no cell is slower by more than 1% with separated
  ranges. Stage 2 passes on its own end-to-end cells (Mastermind; lazy-route fact files with `--json --stats`).

## Alternatives considered

- A `Meter` by type for grounding: a lifetime through every grounding signature, and its charges are ranges a meter
  does not admit.
- A lifetime on the account's control so no controlled account can leave its scope: the same signature cost, for a
  property the debug-build flag already checks.
- Changing the public `Refusal` trait so settlement receives the replaced error: a public change for a need only
  grounding's site-carrying errors have.
- The cadence alone, without settlement: it would loosen precedence.
- Observing at the table's zero requests, or a separate observe operation in its callback protocol.
- Observing by time: polling reads shared flags without a clock read.
- Keeping observation at every charge: it costs 1.7–4.8% in every class profiled.

## Decisions

- The latency contract above (at most 255 charged units after a stop, plus the charge or element in flight).
- Exit settlement through the one entry, including the inverted zero-allowance ordering.
- `zetesis_cpu::poll_due` is public; the table callback contract is restated; the four owners' classifications are
  published.
- Stage 2 follows Stage 1's acceptance and is accepted only on its own gain condition.

## Open review items

- Stage 1's owner methods now classify `ModelError::Catalog` through the catalog owner, and terminal scopes cover
  deferred-storage observation and admission's tail ceilings. Review and qualification must check those actual consumers.
- Before Stage 2 is built, settle the evaluator's and renderer's `Work` by structure (one settled constructor, private
  fields) or state the strength of the listing.
- Label the term-comparison reach column for what it measures, attribute it per profile by the comparison's callback,
  and call `zetesis_cpu::oracle` the CPU oracle.
- Split conjoined test propositions (a zero charge neither observes nor counts; the outranked and passed-through
  refusal pairing).
- Offered: one operation that brackets work with no control hook (observe, run under the remaining allowance, absorb),
  replacing the separate `poll` and `absorb` at the four unbounded sites.
