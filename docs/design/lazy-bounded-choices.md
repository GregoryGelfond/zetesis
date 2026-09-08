# First lazy bounded-choice slice

Status: proposed first implementation slice, 2026-09-08, under the
[execution and source-coverage sequence](parallel-execution-and-source-coverage.md).
The extension described here is not yet implemented or mechanically proved.

**Recommendation:** retain the existing exact normal/choice reduct-closure
engine, and add a separately checked family of original cardinality constraints.
Compile choice support into relational templates; validate each complete
closure against whole source groups before publishing an answer. This admits
`1 {a;b} 1.` without constructing its complete ground-rule or Ferraris DAG store.
It also supports a useful finite relational fragment, including recursive
eligibility, without assuming that the source is tight.

## Smallest useful admitted fragment

Start with normal rules, ordinary headless constraints, and choices
`L { h1 : E1; ...; hn : En } U :- B` where bounds are closed integers, heads use
the existing finite scalar atom patterns, and `B`/`Ei` are conjunctions of
positive/default-negated/double-negated atoms and already-supported static
equality/inequality filters. Require every variable to have the existing positive
binding justification; local element variables and outer group variables remain
distinct. Ground elements are allowed, so the two-atom admission control works.
Strong negation retains the existing coherence obligation.

Closed intervals/fact expansion already handled by the scalar boundary may feed
this representation within the original expansion budget. Do not silently admit
new expression-generated domains, variable aggregate bounds, aggregate-dependent
conditions, weighted/count-tuple head aggregates, general disjunction, or source
functions through this slice. Signed bounds require their actual integer
comparisons: a negative lower bound is vacuous; a negative upper bound forbids an
active group. Reversed bounds with an inactive body are harmless, not a global
UNSAT shortcut. No cast from a negative bound to an unsigned count is valid.

The initial qualified slice can exclude objectives explicitly. Existing lifted
objective scoring is a separate possible addition after source acceptance is
complete; it must not consume an unfiltered base-program model.

## Why separate constraint validation is sound

Let `P₀` denote all normal rules and unbounded choices, with the original bodies
and eligibility conditions retained. For one ground group, let `H` be its
distinct head carrier and `E_h` the OR of all that group's eligibility witnesses
for head `h`. Its count in candidate `M` is

`c(M) = |{h ∈ H : h ∈ M and M satisfies E_h}|`.

The cardinality condition is the original constraint
`C = not (B and not (L ≤ c ≤ U))`. The current eager implementation constructs
exactly this shape in `zetesis-themelios/src/formula_ground.rs::Builder::choice`;
it adds separate choice support implications and one bound constraint.

For any constraint `not F`, `ChoiceIntervals.negation_frozen` proves that its
Ferraris reduct is satisfied by *every* interpretation `J` precisely when
`M` does not satisfy `F`. Consequently, if `M` satisfies all such constraints,
the proper-subset models of the constrained reduct are exactly those of
`P₀`'s reduct. If `M` violates a constraint, it is not an original model.
Therefore, for this constraint family:

`Stable(P₀ ∪ C, M) ↔ Stable(P₀, M) and M satisfies every C`.

This is a direct two-direction semantic argument, not a newly checked Lean
theorem in this review. Existing `FerrarisGuards.stable_guard_theory_iff` proves
the corresponding general filter theorem for double-negated guards;
`ChoiceIntervals.cardinality_constraints_frozen` establishes that both cardinality
bounds supply candidate truth rather than reduct support. A focused source-group
composition theorem should connect these existing results to the new API.

Each choice element becomes the existing normalized rule
`h :- B, E, not not h`. Positive conditions remain positive reduct antecedents;
negative literals remain frozen gates. Multiple eligibility witnesses can remain
multiple producer templates: their consequences combine by OR. They must **not**
be collapsed to unconditional support merely because their original truth is a
tautology. The bound counter, unlike the support-rule stream, deduplicates heads.

`Semantics.stable_iff_gamma`, `accept_sound` and `stable_complete` already give
the exact least-closure/seed characterization for this normalized base.
`LiftedBridge.ConceptualGrounding` explicitly permits a finite conceptual ground
program without runtime materialization. `LazyRounds.completed_round_exact`
requires complete fresh source coverage, sound derivation from empty, and a
completed no-growth round. None of these assumptions is replaced by the
tightness of an observed registry. Positive cycles therefore remain correctly
handled by closure, rather than by an unjustified support certificate.

## Concrete library seams

1. **Source admission:** add a consuming lazy-fragment boundary alongside
   `PreparedFormula::ground`, reusing its owned source, raised themelios program,
   `RuleIr`/`Element` binding scopes, origins and remaining preparation budget.
   Return a source-identified plan containing a normalized consequence `Program`
   plus bounded `ChoiceGroup` descriptors. The existing `compile.rs` refuses
   bounds, multiple elements and conditional choices; bypassing those refusals
   must produce this richer plan, not discard group semantics. Keep private IR
   invariants private and keep the themelios pin unchanged.
2. **Bindings and group scans:** reuse the CPU relation index and iterative join
   implementation. Existing `source::scan` emits `Instance` atoms but no template
   identity or full binding, so it cannot identify a group when an outer variable
   is absent from its head. Introduce a checked binding-view/visitor seam or a
   group-specific scanner over that same join engine. Preserve `(source rule,
   outer binding)` as the group identity. Do not encode hidden group identities
   as synthetic public atoms or copy the eager grounder's large builder.
3. **Membership composition:** `Candidates` supplies complete gate seeds;
   `oracle::check` or `lazy::check_with_source` computes exact base closures.
   `GpuLazyOracle::check_batch_with_source` already supplies real per-candidate
   Metal consequence work for the same normalized templates. A new library-owned
   result requires both base acceptance and completed group checks before a
   whole-source receipt is constructible. Existing core-program receipts remain
   receipts for `P₀`; they must not be relabeled as complete source acceptance.
   The CLI should compose this boundary, not own the group semantics.
4. **Limits and interruption:** source/group scans share cumulative work and
   retain typed exhaustion, cancellation and callback failures. Bound group
   descriptors, retained binding frames, one group's deduplication storage and
   batched check output. Scope completion to source identity, exact closure and
   occurrence; publish nothing from a partly validated group family. A resumable
   cursor must retain that immutable subject and its remaining budget. The
   current callback scanner is exhaustive-or-error, not already a resumable
   group protocol.

## Coverage and memory: what remains complete

The complete *logical* gate carrier remains necessary. `Candidates` enumerates
`Program::gate_atoms` incrementally and records exhaustion separately from a
limit stop. Every possible choice head must occur in that carrier even if no
current closure derives it. Every required frozen gate must remain represented.
Current Cartesian predicate/domain enumeration is complete but can be enormous;
this slice does not claim to solve that candidate-generation problem.

An accepted candidate does not require a retained table of every possible ground
group or eligibility witness. After exact closure `M`, enumerate every outer
binding whose body holds in `M`; separately enumerate all local witnesses true
in that same `M`. Count distinct selected heads per group. Positive joins over
`M` are complete for true conjunctions in the admitted fragment; frozen literals
are checked against `M`. An active outer group must still be checked when the
local join produces **zero** rows. Inactive groups are omitted only because the
complete body test establishes their vacuity.

This distinction matters: one must exhaust the whole group's relevant
eligibility search, but need not materialize all its possible rows. A per-group
set of selected head identities is a straightforward bounded implementation.
A complete finite group carrier becomes necessary if later pruning computes
maximum remaining capacity from undiscovered choices, or if a cached eligibility
table is claimed complete. Current-world prefix coverage is not such a
certificate. Use no bound-based partial-candidate pruning in the first slice.

Choice support source rounds still rescan all templates against fresh closure
snapshots until a complete no-growth round. Positive derivations can reveal new
bindings late; earlier exhausted snapshots do not discharge them. Post-closure
group checks need their own exhaustion receipt. No stable result should be
published during discovery or before that final filter finishes.

## First acceptance and regression tests

- `1 {a;b} 1.` yields exactly `{a}` and `{b}`; an unbounded multi-element control
  keeps all four models. Splitting the bounded group into singleton groups must
  be detected as a different program.
- Relational outer groups with local variables, including an active group with
  no eligible rows and a separate inactive group with impossible bounds.
- Duplicate heads with duplicate and distinct eligibility witnesses count once;
  a head supported elsewhere does not count when this group's eligibility is
  false. Retain the source order/permutation controls.
- `1 {a:a} 1.` has no stable model. Also cover mutually recursive positive
  eligibility, negated eligibility and eligibility disjunction induced by
  repeated heads, including `a`/`not a` witnesses.
- A derived relation enables a choice only in a later source round; multiworld
  Union/Worlds scans cannot mix eligibility truth across candidates.
- Exact/one-short limits at outer discovery, last local row, dedup growth, device
  transfer, completed closure, final group and result publication. Verify no
  missing group becomes a successful zero-row completion after interruption.
- Exhaustive tiny source/eager equivalence, original and arbitrary frozen `M/J`
  formula checks for normalization, clingo complete answer sets, and matched
  scalar/Rayon/Metal results. Preserve duplicate candidate occurrences and
  distinguish candidate exhaustion from checked-model completion.

Finally compare a sparse relational fixture in eager/lazy CPU/Metal modes,
recording generated instances, gate-carrier discovery, peak catalog/group
payload and all source/checking phases. Avoid a speed or RSS claim based solely
on omitting a ground-rule vector. This is a bounded general extension of the
existing reduct engine, not full lazy formula support or closure of the 94-case
corpus gap.
