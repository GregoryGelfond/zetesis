# Fact path: oracle preparation by identity, unchanged statements kept, one admission

A fact is a body-free ground rule. This note designs three independent parts that cut the per-fact cost of the paths
facts take besides the [columnar fact blocks](columnar-fact-relations.md), which take facts out of the formula route's
rule population. None changes a program's answers; each saves elapsed time and allocation on fact-heavy inputs.

## Status

| Part | Status |
|---|---|
| Part 1: oracle preparation finds domain positions by identity | Implemented |
| Part 2: statements normalization leaves unchanged are not rewritten, on both routes | Implemented |
| Part 3 | Withdrawn: subsumed by columnar fact blocks |
| Part 4: one raise and one profile walk per invocation, behind one automatic admission per door | Open |

Paths are under `crates/` unless another root is named.

## Problem

zetesis spent 4.6 µs on a plain fact on the lazy route and about 12 µs on the formula route, where clingo's whole run
spends under 2 µs per fact. Measured against clingo 5.8.2 at 16,000 facts, before columnar fact blocks:

| Shape | Route | zetesis | clingo | Ratio |
|---|---|---|---|---|
| unary facts | lazy | 74.0 ms | 30.0 ms | 2.5× |
| facts with function and string arguments | lazy | 269.4 ms | 79.3 ms | 3.4× |
| unary facts and one comparison rule | formula | 194.6 ms | 30.3 ms | 6.4× |
| one range fact `p(0..15999)` and one comparison rule | formula | 115.4 ms | 12.0 ms | 9.6× |

One arithmetic comparison anywhere sends a program to the formula route: the extended compile refuses it as not
ground and the CLI prepares the program again (`zetesis-cli/src/admission.rs`; `ExpansionFailure::needs_formula_admission`
in `zetesis-themelios/src/expansion.rs`). Three per-fact costs are this design's:

1. **Content comparisons where identities are known** (lazy route, 1.25 µs per plain fact, 5.3 µs per wide fact):
   preparing the membership oracle binary-searched each constant head term in the sorted domain by content on every
   fixed-point pass, and ranked each ground head value on its dense axis by another content search. (Part 1)
2. **Rewriting statements normalization leaves unchanged** (formula route 1.2 µs per fact; 0.13 ms of a tiny corpus
   run): the formula compiler cloned and rewrote each statement, and the extended route's skip did not cover ground
   function terms. (Part 2)
3. **Admitting twice** (formula route, up to 1.1 µs per fact): on the bundle door the extended admission walks the
   profile, raises, collects metadata, clones and ingests the statements before `extended::compile_owned` refuses;
   formula preparation then walks the profile, checks definitions, counts and objectives, raises and ingests the same
   sources again. (Part 4)

## Part 1 (implemented): oracle preparation by identity

`zetesis_core::DomainPositions` is a domain position table: one `Option<NonZeroU32>` slot per term of the program's
catalog (4 bytes per term), holding the term's domain position plus one, or `None`. Term identifiers are dense, so a
slot is indexed by `TermId::position`; a lookup resolves the value through the program's own read and reads its slot,
and anything else (`None`, or another catalog's term) falls back to the content search, which stays the authority. A
domain of `u32::MAX` values or more gets no table, never a refusal. Oracle preparation (`zetesis-cpu`
`oracle/prepared.rs`) builds the table after the rule index, lends it to argument-bound inference (constant head terms'
positions are added once, before the fixed point) and to head-coordinate preparation (an integer search on each axis),
and drops it. While it lives, its reserved capacity counts against `PreparationLimits::max_bytes`, and its refusals are
preparation's existing storage stops. The closure's ranking is unchanged.

Correctness rests on the interner's invariant that equal canonical terms share one identifier, and on the domain
holding each value once in canonical order. Measured: preparation work falls 78–92% on 200-fact shapes, retained bytes
unchanged, answers and output identical on the corpus, scalability, example and fact-shape programs on both routes.

## Part 2 (implemented): keep unchanged statements unchanged on both routes

`kept_as_is` (`zetesis-themelios/src/extended.rs`) extends its leaves from argument-free scalars to ground canonical
symbols with arguments none of whose parts names a `#const` definition, parameterized by the route's normalizer.
Raise yields canonical statements, and normalization's symbolic branch returns such a symbol unchanged, charging one
unit, its payload bytes and one unit per part; the kept path charges exactly that, in the same order, so receipts are
unchanged and a failing statement raises the same failure first. On the formula route the skip applies to fact
statements only, and each expanded fact receives the provenance `rewrite` would stamp at statement level
(`Transformed("zetesis-finite-formula")` merged into the carrier's), so every retained provenance is identical.

## Part 4 (open): one raise and one profile walk per invocation

**The equivalence it rests on.** At the pinned themelios revision, `raise(parse)` is `lower_each` followed by
`collect_runs`, and `raise_occurrences(parse).into_raised()` performs the same two steps (themelios documents the
contract); the formula route already raises this way (`formula_raise.rs`).

**Entry points, one per door.** One library sequence owns the route decision the CLI makes today for a single source
and for a bundle, and that the engine behind the facade makes for an already-admitted program
(`zetesis-engine/src/prepared.rs`). Each door gets its own public entry point, with its retained counterpart's
parameters and the shared generic result types instantiated with that door's owners and refusals:

```rust
pub fn admit_automatic_with_cancellation(
    source: ParsedSource,
    expansion: ExpansionLimits,
    formula: FormulaLimits,
    cancellation: &Cancellation,
) -> Result<
    AutomaticAdmission<Admitted, PreparedFormula>,
    AutomaticAdmissionFailure<SourceFailure<ExpansionFailure>, SourceFailure<FormulaFailure>>,
>;

pub fn admit_bundle_automatic_with_cancellation(
    bundle: SourceBundle,
    options: BundleAdmissionOptions,
    expansion: ExpansionLimits,
    formula: FormulaLimits,
    cancellation: &Cancellation,
) -> Result<
    AutomaticAdmission<AdmittedBundle, PreparedFormulaBundle>,
    AutomaticAdmissionFailure<BundleAdmissionFailure, FormulaBundleFailure>,
>;

pub fn prepare_program_automatic(
    program: Arc<Program>,
    options: ProgramRelationalOptions,
    formula: FormulaLimits,
) -> Result<
    AutomaticAdmission<PreparedRelational, PreparedFormula>,
    AutomaticAdmissionFailure<FormulaFailure, FormulaFailure>,
>;

pub enum AutomaticAdmission<E, F> { Extended(E), Formula(F) }
pub enum AutomaticAdmissionFailure<ER, FR> { Extended(ER), Formula(FR) }
// One inherent `interruption() -> Option<Stop>` per door's instantiation.
```

One private generic function, bounded by a crate-private door trait, holds the sequence: attempt the extended route;
return its admission; on an interruption or a refusal for which `needs_formula_admission` is false, return it with its
owner; otherwise prepare the formula route on the same input. `interruption()` classifies both sides (on the extended
side it takes over the classification the CLI makes today), so a caller never reads an interruption as a refusal. The
existing extended and formula entry points keep their contracts and signatures. The program door has no handoff: its
second attempt repeats an inspection, not a raise, so it gains one home for the rule rather than time.

**The CLI.** Under `--oracle auto` each door function first evaluates `validate_formula(options)`, a check of option
values that allocates nothing (under lazy or automatic CPU grounding it refuses shared source batching for the formula
route). If the formula route is permitted the door calls the entry point; otherwise it calls the extended entry point,
exactly as under `--oracle closure`, and a refusal needing formula admission returns today's policy refusal. This keeps
today's order exactly and needs no API change. `--oracle countermodel` prepares the formula route directly, as today.

**Inside the extended attempt, per source, in today's order.** One profile walk computes both profiles' verdicts (the
extended and formula profiles are the same traversal under different flags, `profile.rs`), returns the extended verdict
and records the formula verdict (its node count or first refusal); definitions and the metadata count run under the
same limits the formula route uses; `raise_occurrences` runs once; formula metadata is collected from the occurrence
stream into a builder under `formula.metadata_storage`, a refusal recorded with its source index; `into_raised()`
supplies the extended program.

**The handoff** (private to the entry point) holds the program (the ingested program on the bundle door, the raised
program on the single-source door), per source the formula profile's node count, and the formula metadata builder or
its deferred refusal. It is taken only on a refusal from the extended compile for which `needs_formula_admission`
holds, and only when every recorded formula verdict passed (the guard). An earlier refusal needing formula admission
prepares the formula route without a handoff, as today.

**Formula preparation with the handoff is formula preparation itself.** `prepare_source` and `prepare_bundle` take an
optional handoff, and each step either computes as today or reads its recorded outcome in place: the polls run; the
profile check advances `visited` by the recorded node count; definitions and count read their recorded outcome;
`check_objectives` runs; the raise step returns a recorded formula metadata refusal where the raise would have returned
it; after the loop the handoff's program takes the place of the second build. Because the loop is formula
preparation's own, the first refusal in today's order is returned by construction.

**The profile lemma.** Under the same options and includes treatment, a source the extended profile accepts is accepted
by the formula profile with the same node count, and holds no weak constraint or optimize statement: the traversal
counts the same nodes under both flags; the formula flag only widens head forms, body elements and observations; with
the formula flag unset `metadata::check_syntax` refuses every body-carrying `#show` and `#project`; and an objective
statement is refused as unsupported by the extended profile. So on the handoff path every recorded formula verdict
passes and the objectives check passes. The lemma is pinned by a test; the guard holds the handoff to it at run time,
so a later profile change costs time, never a skipped check or a refusal out of order.

**Costs, accounting and reach.** On the bundle door's fallback path Part 4 saves the second profile walk, the scans,
the raise, the statement clones and the ingestion: up to 1.1 µs per fact, about 18 ms at 16,000 facts with a
comparison rule (the walk 3.5 ms, the raise 11.2 ms, the ingestion 2.9 ms). The single-source door (standard input,
`run_with_renderer`) saves the walk, scans and raise. On the lazy route it costs the formula flags inside the shared
walk and one formula metadata pass, O(directives), to be measured on large fact files. Neither the raise nor the walk
charges the expansion budget, so no work receipt changes. The reach report gives the share of `--oracle auto`
invocations per door that take the handoff.

**Tests.** The route chosen and every refusal's diagnostics are unchanged on every corpus program and refusal fixture;
the recorded formula verdict equals `check_formula`'s; the profile lemma on every corpus program, fixture and generated
source; the guard (a crate-internal seam records a failing formula verdict, and the entry point prepares from scratch
and returns the reference's result); a formula metadata refusal in source 2 of 3 on the bundle door and one on the
single-source door, each asserting the handoff was taken; the reference is the composition of the retained public
entry points; an interruption during the extended attempt is reported with no formula attempt; the formula-policy
fixture under `--source-batching worlds` with lazy and automatic grounding (today's policy refusal, today's extended
refusal for a refusal not needing formula admission, and a pre-cancelled run reported as an interruption on both
doors); the engine's tests unchanged; `--oracle auto` output byte-identical on the corpus; `raise` equals
`raise_occurrences(...).into_raised()` on every source, compared by `Debug` as well as `==`.

**Acceptance.** Against the parent's release front: interleaved A/B of process time on the fact shapes at 4,000 and
16,000 facts (unary facts with one comparison rule on both doors, through a file and through standard input), the lazy
route's delta on large unary and wide fact files reported, hyperfine on a tiny corpus case, then the corpus at one and
fourteen workers; answer families identical on the portable, oracle and scalability suites.

## Alternatives considered

- **Part 1:** a hash map from identifier to position (8–16 bytes per value and a new reservation path); sorted
  (identifier, position) pairs (8 bytes per value and a logarithmic search; the fallback if terms greatly outnumber
  domain values); owning the table in the core program (charged at every core admission, needed by none).
- **Part 2:** dropping the transformation stamp (observable through `analyzed_program()`); re-stamping nested carriers
  (rebuilds what the formula route discards).
- **Part 4:** one function over a sealed door trait (the workspace's first sealed trait, needing a lint exception) or
  enums whose arms each caller must match although its input cannot produce them; relying on the profile lemma without
  the guard.

## Decisions

- Part 4 has one public entry point per door, sharing generic result types; the door trait is crate-private.
- Part 4 covers the program door's route decision.
- The CLI's formula-policy check stays in the CLI, evaluated before the call.
- Part 1's table is owned by oracle preparation as preparation scratch.
- Part 4's handoff is taken only when every recorded formula verdict passed.

## Open review items

- **Who builds Part 1's table.** State it by the callers of CPU oracle preparation: the closure route's narrowing for
  any program whose `gate_predicates()` is nonempty (under any grounder and backend), the CPU lazy executor when it does not adopt the
  narrowing's preparation, and the one-shot check; state that the formula route and the shared-batching executor never
  build it. Name the narrowing's boundary (its stop is recorded as `narrowing_stop`, enumeration proceeds un-narrowed,
  answers unchanged) and the one-shot check's ledger (`Limits::max_closure_bytes` and its statistics); correct the
  charge and accounting statements accordingly; test both cases; report reach by caller and measure one shape with a
  nonempty `gate_predicates()`. (This concerns the implemented Part 1's statements and tests.)
- State the costs and charging of Part 1's public type, with two in-code contracts.
- Make the single-source door a `ParsedSource` method, or name the departure; settle the program door's duplicated
  limits.
- Make the extended attempt one sequence with an optional recorder.
- State what the guard covers (formula-only profile restrictions; the objectives step covers objective statements) and
  the lemma test's population; update the manual sentence.
- State both directions for the work ceiling.
