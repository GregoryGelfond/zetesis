# Grounding without repeated work: unaffected rich producers and shared local join plans

Support completion computes the possible atoms in rounds over the producer rules; final emission then joins every
rule once over the completed support and builds the formula theory. This note removes two repetitions inside that
process and records a roadmap for two more. Each stage is an execution choice that preserves the support fixed point
and the emitted theory exactly; the [grounding chapter](../book/architecture/grounding.md) describes the schedules.

## Status

| Part | Status |
|---|---|
| Stage 1: one registration per producer, prepared once; an unaffected rich producer is not traversed | Implemented, with the receipt `support_unchanged_visits` |
| Stage 1's receipt `support_rich_traversals` | Implemented; counts actual rich traversals, including the first round |
| Stage 2: one plan per local frame per snapshot, owned by the snapshot, consulted only through `Join::local` | Implemented; acceptance measurements remain open |
| Stage 2's receipts `local_plan_requests`, `local_plan_preparations` and `local_plan_reuses` | Implemented; preparations and reuses are disjoint subsets of requests |
| Stage 3: share a rich producer's traversal between support and emission | Roadmap |
| Stage 4: one ordered pass on the direct route | Roadmap |

Stages 1 and 2 leave the support catalog, the emitted theory and the answer output unchanged; the statistics output
changes by design (new receipts, lower work where traversals or plans are saved). Paths are under
`crates/zetesis-themelios/src/` unless another root is named.

## Problem

Measured sample shares, support / emission, at one worker before stage 1:

| Program | Rounds | Support | Emission | Assignment values, support / emission | Choice, support / emission |
|---|---|---|---|---|---|
| traveling-salesman variant 05 case 05 | 12 | 37.0% | 17.4% | 3.4% in all | 13.1% / 2.6% |
| shortest-path variant 04 case 06 | 8 | 31.5% | 16.0% | 11.3% / 1.8% | 2.4% / 0.5% |
| nested Mastermind | 5 | 54.0% | 27.4% | 35.2% / 11.9% | 0.5% / 0.5% |
| sudoku | 3 | 12.6% | 66.4% | — | support's rule work |
| shortest-path variant 01 case 01 (tiny) | 4 | 12.3% | 9.3% | 2.0% in all | — |

1. **Rich producers re-traversed with unchanged inputs (stage 1).** A producer plan, which wakes only rules whose
   inputs changed, exists only for positive-flat programs without objectives (`formula_support/producers.rs`,
   `formula_domains/positive.rs`). Every maintained eager encoding with a choice rule runs `Schedule::all`, which
   visits every producer in every round. A *rich* producer (a choice, disjunctive or conditional head, or an
   aggregate, conditional or projection in its body) took a full traversal on every visit, even when nothing it reads
   had changed: its body join, every per-binding element join and aggregate proposal, and every head, each already
   supported. Every visit also re-classified the rule (`delta::variants` scanned the body, about 0.9% of the TSP
   case's samples). Facts are not producers: a fact block is interned in round 1 and has no producer slot.
2. **Per-binding local join construction (stage 2).** Each outer binding of a choice rule, conditional head or
   assignment aggregate builds a fresh `Join` and plan per element, although within one snapshot the plan is the same
   for every binding of one site with one bound-slot pattern. `Plan::new` + `with_plan` take 5.2% + 4.3% (TSP case),
   5.9% + 7.8% (nested Mastermind) and 1.9% + 1.4% (tiny shortest-path), whole-process shares that include outer
   plans.
3. **One populated traversal per rich producer in each pass (stage 3).** Support derives a rich producer's possible
   heads and emission builds its formulas over the same rows.
4. **The direct route joins every rule twice (stage 4):** support completion and exact activity, each about a third
   of prime-sieve(300)'s samples.

## Stage 1 (implemented)

**Registration.** On the full-schedule path each producer gets one registration, prepared before the first round and
retained for the completion. It records once what the per-visit classification derived:

- a **certified** producer (normal head, no projected, conditional or aggregate body literal) registers its positive
  body occurrences, as the producer plan's prepared inputs do;
- a **rich** producer registers its **read set** `R(r)`: the distinct predicates of every atom occurrence, of either
  sign, in its body and in its head elements' conditions, at any depth (aggregate elements and keys, conditional
  literals' conditions and consequents, projections' predicates and witness bindings);
- a **constraint** is recorded as such; `Schedule::all` never yields it.

The walk is one enumeration of the atom occurrences a frame holds (`formula_support/occurrences.rs`), exhaustive over
the IR's head, literal, consequent, projection and aggregate-key forms with no wildcard arm, and shared with
`Demand::of`, so the two consumers cannot disagree. A read-set entry is a compiled coordinate, deduplicated by sign,
name and arity; the registration's bytes count as the catalog's prepared bytes for the completion.

**The test.** Round 1 traverses every producer. In later rounds a certified producer takes its registered inputs'
delta variants without the per-visit scan, and a rich producer is traversed exactly when some predicate of `R(r)` has
new rows in the round's snapshot (`old_rows < row_count`). The test stops at the first changed predicate and charges
one unit per entry tested plus the resolution's unit. `support_unchanged_visits` counts the rich-producer visits that
ended at the test. A test-only, thread-local switch restores full traversal for in-process reference runs.

**Correctness.** A traversal of `r` in round `i` reads (a) the relations of `R(r)`'s predicates through its outer
join, element joins, assignment proposals and group validation, and (b) the support membership of the atoms it
derives, only to prune a supported head or inside `retain`. Its plan depends only on `R(r)`'s row counts, and nothing
else it reads varies between rounds (domain guards exist only for producer-plan programs; an assignment's required
tuples are fixed in the IR; integer comparison forms decide rows from their values; fact rows are published once). If
`r` was traversed in round `j` and is unaffected through round `k`, then `S_k` and `S_j` agree on `R(r)` (publication
sets old rows before appending), membership only grows, and a round-`k` traversal would visit a subset of round `j`'s
bindings, discover nothing and retain nothing. Skipping it leaves every pending delta, identity, publication and the
fixed point unchanged; completion is still detected by an empty delta. Round 1 traverses every producer, which a
producer with only negated body literals (`{a} :- not b.`) needs.

The premise the code must keep is that `R(r)` covers every relation (a) lists; tests protect each kind of member.

**Lean.** `proofs/Zetesis/ProducerScheduling.lean` is generalized over stable producers: a producer family has an
abstract proposal function; `selective_step_exact_of_stable` and `empty_selection_closed_of_stable` add a stability
premise (an unaffected producer proposes nothing over the current carrier it did not propose over the old one);
`stable_of_reads_only` proves stability for a producer whose proposals depend only on the carrier's restriction to its
registered predicates, when the carrier grows. The earlier theorems keep their statements as corollaries.
The Rust correspondence remains an obligation. Its abstract proposal function must be the complete set of heads
before membership pruning, not the carrier unioned with those heads. The registration must cover every relation
that function reads. Separately, pruning against published and pending heads must preserve their union with the
proposals. Publication grows the carrier. The [correspondence guide](../../proofs/guide/producer-scheduling.md)
states these obligations; differential tests compare the implementation with full traversal but do not prove them.

**Accounting** (work receipts measure the work performed):

| Receipt | Stage 1 | Stage 2 |
|---|---|---|
| `support_production_work` | Falls with skipped traversals and the removed scan; rises by at most two units per read-set entry per rich visit after round 1 | — |
| `support_join_work`, `support_head_work`, `join_rows`, `binding_snapshots` | Fall with skipped traversals | — |
| `support_producer_visits` | Unchanged (every visit is recorded before the variants) | — |
| `support_unchanged_visits` | Rich-producer visits that ended at the test | — |
| `support_rich_traversals` | Rich-producer visits that traversed, including round 1; disjoint from unchanged visits | — |
| `local_plan_requests`, `local_plan_preparations`, `local_plan_reuses` | — | Local constructor attempts; inserted plans and existing-entry hits are disjoint subsets |
| Expansion `TermWork` | Falls with skipped traversals | Falls by `Plan::new`'s charges per reuse; rises by lookups and insertions |
| `support_peak_bytes` | Rises by at most the registration; falls where a skipped traversal set the peak | Rises by at most the plan table |
| Work and byte ceilings | Either direction under explicit limits; no refused↔completed flip at the default limits | The same |

Summed over a completion, the candidate's formula work exceeds the baseline's by at most the preparation plus
`Σ max(0, 2|R(r)| − |body(r)|)` over rich visits after round 1 (a skipped traversal can cost less than its test, for
example `{q(X) : a(X), b(X), c(X)} :- t.` reads four predicates with one body literal).

**Measured effect** (deterministic receipts, no timing): `support_unchanged_visits` 30 (TSP case), 18 (shortest-path
variant 04), 8 (nested Mastermind), 3 (sudoku), 2 (n-queens 8), 4 (tiny shortest-path); support production work
−59%, −63%, −57%, −34%, −33%, −26%; join work −56%, −74%, −63%, −50%, −51%, −37%; rounds, producer visits and peak
bytes unchanged. End to end at one worker on a quiet host: nested Mastermind −27.5%, the TSP and shortest-path cases
about −12%, sudoku −7%. Answers and text output are identical on the corpus, scalability and example programs, also
with the formula route forced.

## Stage 2: one plan per local frame per snapshot

The implementation is present; the release measurements below remain its acceptance gate. A **call site** is a
constructor invocation in the code; a **frame** is one live nonempty literal slice. Several call sites can use the
same frame, and share its first admitted plan within one snapshot.

**Exact inputs.** `order::Plan::new` reads the literal contents, variable count, prefix length and every bound bit,
the snapshot's component view, relation row counts, and pivot. Local joins always use full rows (`pivot = None`).
The key borrows the literal slice for the snapshot lifetime and compares address, length and variable count; the
entry additionally matches the original prefix length and every bit. Actual bound values, caller location, head
staging and later query strategy are not successful-plan inputs. Plan construction still diagnoses unsafe prefix
slots before publication. New planner inputs must be added to this contract and its identity tests.

**Owner and invalidation.** `Support` owns a lazy plan table beside its relation-table workspace. A support round
reads its immutable relations while writing discoveries to a separate pending delta. Each published round creates
another `Support`, with an empty plan table; completed queries similarly create and drop their own table. Source
components and offered row counts cannot change within that owner. No process-global cache, completion token or
cross-worker table is involved.

**Whole lifetime path.** The borrowed `Ir::Source` carrier preserves the snapshot-lived source IR through source
rule constructors, `Join.literals`, each binding cursor's generators, aggregate assignment values and their element
conditions. `Ir::map` preserves that lifetime when selecting nested IR. Source emission carries the same borrow
through aggregate, projection-witness and conditional lowering. `Ir::Temporary` uses the identical lowering and
fresh-plan construction for temporary factored IR and scoped scratch whose input does not promise the snapshot
lifetime. Converting an ordinary reference into the carrier chooses Temporary; inserting a Source key requires the
longer borrow explicitly. No IR payload is copied for the cache.

A live join holds an `Rc` to its immutable plan and borrows the `Support` that owns the table. This lets nested local
joins fill the table while earlier plans are live. The cell is borrowed only for lookup/insertion, never during plan
construction or traversal. `Rc` makes `Support` neither `Send` nor `Sync`. Worker checkers retain `CompletedSupport`
and prepared outer plans; they construct query workspaces locally for a check. A shared local plan is never
re-decided: the factoring component constructor keeps an owned plan.

**Served sites.** `Join::local` is the only constructor that consults the table. `Join::element` and `Join::local_head`
forward to it. Explicit source carriers serve support's conditional/choice heads, keyed-group validation and nested
assignment values; source emission's projection witnesses, aggregate elements and conditional literals; arithmetic
admission's local fragments and heads; source activity's choice elements; and objective presence's local elements.
Empty frames and temporary carriers bypass the table. A frame/variable-count pair with another prefix shape builds
an owned plan without replacing the first entry. Outer rule/variant/objective plans and prepared outer plans never
consult the table themselves, even when their nested source assignments do.

**Work and storage.** The address-sorted entry vector has `O(log F + P)` lookup and `O(F + P)` insertion work, for
`F` recorded frame/variable pairs and `P` prefix slots. Expansion `TermWork` counts actual binary-search comparisons,
prefix-length/bit inspections, copied prefix bits and entries shifted on insertion. Existing fallible reservation
also charges its actual allocation/copy work to the ordinary workspace account. Pointer ordering can therefore
change planning charges with heap layout; this is an explicit limitation for bounded runs, not a claim of stable
work totals across allocators. A successful plan's row order is unchanged. Default-limit refusal identity remains
an acceptance check. Exact work-boundary tests therefore retain one borrowed frame owner while resetting the table
and budget; source preparation has its own exact boundary before any local table is used. Compiled frame IDs are
a follow-up option if measurements justify making local-plan lookup work independent of heap layout; changing to
linear lookup would discard the current lookup bound.

The snapshot's existing workspace lease counts entry-array capacity, bound-bit payload and one `Rc` allocation per
plan (count words, plan header and its vector capacities). Shared joins charge their handle in their own header but
no second plan payload. Planner scratch is checked before each allocation; a completed unpublished plan stays
accounted while its entry is reserved. Refusal inserts nothing and releases that unpublished storage. Requested
capacity is not reported as allocated peak. The table releases its receipt with the snapshot.

**Reach and reference.** `local_plan_requests` counts every local constructor attempt, including empty/temporary
frames, prefix-shape fallbacks and later refusals. `local_plan_preparations` counts successful insertions;
`local_plan_reuses` counts matches to existing entries. Preparations and reuses are disjoint subsets of requests;
a reuse still counts when later join setup or traversal fails. Requests are the denominator, not an estimate of
saved setup time: `Join::with_plan` continues to build independent binding/traversal state.

The scoped test reference disables only local-plan sharing, restores on unwind, and is confined to its thread. A
separate test instrument records the file/line of hits, including nested assignment-cursor hits. Focused tests
compare exact support catalog order and theory nodes/operands/roots with that reference, full binding order and
comparison readiness, exact key/prefix identity, nested insertion, empty/outer bypass, failure publication and
one-entry storage refusal. Acceptance must additionally exercise every served site and the whole maintained corpus
at one and fourteen workers; focused tests alone do not establish that coverage or a speedup.

## Stages 3 and 4 (roadmap)

**Stage 3, replay.** Support's base join prunes a subtree once the head is bound and supported, for every rule with a
normal head, so a support traversal's rows are complete only for producers that do not prune by head projection
(choice, disjunctive and conditional heads, and normal heads whose variable only an assignment generator binds).
Support also hands its consumer only passing rows, while emission validates failing rows too. A replay therefore
records below row selection, passing and failing, for producers inside both boundaries, and emission builds and
validates from the record instead of re-joining (about `rows × slots × 4` bytes per producer until emission). Atom
numbering, and so model order, may change; stage 3 is accepted on identical answer families. Emitting during support
is not proposed: support cannot know a traversal is its last before the fixed point.

**Stage 4, one ordered pass on the direct route.** For complete signed acyclic programs, rules in dependency order can
share each rule's join between support derivation, original-family arithmetic admission and exact activity, removing
one of the two joins per rule. It follows columnar fact blocks, which change how facts enter that route.

Each owes its own design and preservation argument.

## Acceptance (each stage)

On a quiet host, against the implementation's parent built as a release front, with the receipts re-measured there
first:

1. **Answer identity** on the corpus, scalability and example programs at one and fourteen workers (answer sets, costs
   and optimality verdicts); in-process, the support catalog and emitted theory identical.
2. **No refusal flip** at the default limits; a flip under another profile reported with both work totals.
3. **Estimates held to account** by the reach report (rounds, unchanged visits and rich traversals for stage 1; plans
   requested, prepared and reused for stage 2).
4. **Whole-run gain:** end-to-end time falls with separated ranges (hyperfine, 25 runs, alternating) in every cell whose
   estimate is at least 3% of the run.
5. **No regression** above 3% with separated ranges in any grounding A/B, end-to-end or corpus cell.

## Alternatives considered

- Extending the producer plan to every program (its wake sets come from the positive-flat certificate).
- Semi-naive variants for rich producers (aggregate carriers do not decompose by occurrence).
- Recording all support bindings for emission (pruned traversals record incomplete rows).
- A single gringo-style pass (the end state; it moves the builder into the fixed point).
- Consulting the table inside `Join::new` (a new outer site would consult it silently).
- A dense site number in the IR (a compiler change at every site; the borrowed-slice key gives the same identity).
- Deriving `demand::read_predicates` from the shared walk (it changes the hybrid route's retained relations; needs
  its own measurement).

## Decisions

- Stage 1 reports `support_unchanged_visits`; rich traversals are counted by a public receipt,
  `support_rich_traversals`, not by a census outside the solver.
- A later stage may change atom numbering, and so model order, when answer families are identical.
- Stage 2's plan table is owned by the snapshot and consulted only through `Join::local`.
- Stage 4 follows columnar fact blocks.

## Open review items

- Bind stage 1's evidence to the implementing commits.
- Bind stage 2's full-site/reference tests, default-limit checks, reach denominator and release timing evidence to
  the exact candidate and baseline sources/binaries. Its implementation and focused tests are not performance
  acceptance by themselves.
