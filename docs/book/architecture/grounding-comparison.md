# Grounding compared with clingo

The useful distinction is when instances are generated, what is retained and
how inference is scheduled. Both an eager and a lazy implementation can be
described relative to a finite ground program; only one may need to materialize
that complete program. Neither scheduling choice changes zetesis's reduct-based
acceptance criterion.

## Compare execution boundaries

| Route | What happens before membership checking | What must be complete |
| --- | --- | --- |
| clingo ordinary grounding | Selected source parts are instantiated and simplified into ground input for solving | Grounding of those selected parts in their parameter/incremental context |
| zetesis eager formula | Possible support is completed and a finite atom catalog, shared formula DAG, objectives and observations are constructed | Source lowering and every relevant original formula/metadata obligation |
| zetesis hybrid formula | Complete support, arithmetic admission and producer-core materialization precede source-constraint checks against core answers | Core membership and every streamed constraint before acceptance or scoring; complete enumeration, or exhaustive optimal search under sound objective bounds |

These routes do not have equal language coverage. The
[language reference](../reference/language.md) identifies the current profiles.
The hybrid formula profile retains bounded choices and other producers
eagerly. It streams eligible ordinary constraints and scores objectives only after
those checks complete. Core membership uses the selected CPU or GPU executor;
source joins and constraint checks run on the host. Table joins remain unsupported in this profile. General
demand-driven formula producers are not implemented.


Three operations must remain distinct. **Source instantiation** produces bound
rule instances. **Candidate generation** proposes interpretations, or gate seeds
whose closures will supply interpretations. **Positive inference** derives atoms
under a frozen seed. The reference composition below keeps instantiation and inference apart; it does
not identify a proposed atom with a derived atom or eliminate the candidate
generator's coverage obligation.

## Read grounding as a composition

This section and the next state the composition that the source-coverage and
gate-projection laws prove. The relational closure route that executed it directly
was removed in 0.7.0. The composition remains a mathematical account of that
schedule, not the implementation selected by `--grounder lazy`. The current
hybrid route materializes producers and streams eligible constraints, as the
table above shows.

The following notation emphasizes the [exact transforms](alignment.md), not a
particular loop nest. `∘` denotes composition, read from right to left. `Map`
preserves independent subjects; `Reduce` combines results using the stated
operation. `CloseFromEmpty` denotes inflationary consequence iteration through a
*complete* no-growth round. These are descriptions of contracts, not a claim that every name below
is an exported Rust combinator. Every operation may instead stop explicitly;
an unfinished stream cannot supply a fixed-point or exhaustion result.

For eager grounding, the conceptual boundary is:

```text
source parts and parameters
    |> AnalyzeDependencies
    |> InstantiateAndSimplifyToCompletion
    |> RetainGroundInput
    |> Solve
```

This is a boundary sketch for ordinary gringo use, not pseudocode for its full
algorithm. Internally, instantiation already uses indexed binding, component
scheduling and evolving grounding domains. Simplification can change the
representation emitted to the solver. Completion concerns the selected parts
and context, not every part that an incremental application might later request.
zetesis's eager paths have the same broad materialization boundary while
retaining their own formula representation.

For the former relational closure profile, the source operation was:

```text
Offer_t(X) = Instantiate_t ∘ Filter_t ∘ Bind_t (X)
Offer_P(X) = Concatenate(Map(Offer_t(X), templates(P)))
```

`Bind` joins positive witnesses and checks variable agreement. `Filter` evaluates
admitted scalar conditions. `Instantiate` retains the head, positive antecedents
and symbolic gates. A fact has an empty positive body, whose binding is the unit
substitution, so it can contribute even when `X` is empty. Source scans cover
constraints as well as headed rules.

A shared round then composes that offering with world-specific evaluation.
Here `S[w]` is the immutable gate seed, `X[w]` the immutable derived snapshot
and `V[w]` the retained constraint flag for world `w`; `C` is one bounded chunk
of offered instances.

```text
Enabled_w = Gate(S[w]) ∘ PositiveTruth(X[w])
Heads_w(C) = Reduce(Union) ∘ Map(Head) ∘ SelectHeaded ∘ Enabled_w (C)
Bad_w(C)   = Reduce(Or) ∘ Map(True) ∘ SelectConstraints ∘ Enabled_w (C)

Round_S(X, V) =
    Offer_P(Reduce(Union, X))
    |> BoundedChunks
    |> Map(C => MapWorlds(w => (Heads_w(C), Bad_w(C))))
    |> ReduceWorldwise((Union, Or))
    |> RequireCompleteSourceAndEvaluation
    |> MapWorlds((w, H, B) => (X[w] union H, V[w] or B))

closures, violations = CloseFromEmpty(
    Round_S, initial = (all empty, all false),
    complete_when = the completed round adds no head)
verdicts = MapWorlds(w =>
    Accept(closures[w]) iff
        not violations[w] and GateProjection(closures[w]) = S[w])
```

`Head` is projection of a retained instance; constraints have no head and go
through the separate Boolean reduction. `PositiveTruth` is essential: a union
join can offer a binding whose antecedents belong to different worlds. It is
neither a proof of truth in any one world nor permission to mix their seeds.
`Gate` consults only `S[w]`; it does not change as `X[w]` grows.

Precisely, an instance is enabled when its positive antecedents are a subset of
`X[w]`, its true-gate atoms are a subset of `S[w]`, and its false-gate atoms are
disjoint from `S[w]`. If `G_P` is the admitted program's gate carrier, then
`GateProjection(X) = X ∩ G_P`. The closure route's separate `Candidates` generator, removed with that route,
owed complete coverage of the relevant seeds over that carrier.

All chunks in one round read the same snapshots. Chunk results can be reduced
with associative union and disjunction; their new heads become source witnesses
only in the next round. Retaining a triggered constraint is sound because
positive truth grows under fixed gates. A successful no-growth round must have exhausted the
source scan and all required evaluations. Only then do sound derivation from
empty, closure, constraint checking and seed agreement establish acceptance.
An interrupted round supplies progress, not a negative answer or a completed
check. The former shared implementation returned no completed candidate checks
if that batch's required work stopped.

This exposes two kinds of parallel work: common source instances can be reused
across worlds, and enabled instances can contribute through independent truth
tests and reductions. It also exposes the ordering that must remain: a later
round depends on the prior round's completed consequences. World-membership
masks may narrow offered bindings while preserving every world's enabled ones;
they change source work, not the formula above or candidate coverage.

The implementation connected at the closure route's `source::scan`,
`lazy::check_with_source`, shared Rayon evaluator and wgpu evaluator, all removed
with that route.
The source scan and round coordinator ran on the host; the injected evaluator
executed the per-world positive/gate tests and head/constraint reductions.
The composition is meaningful without claiming that source joins run on the GPU.

[`LazyRounds.lean`](https://github.com/GregoryGelfond/zetesis/blob/main/proofs/Zetesis/LazyRounds.lean)
formalizes the corresponding obligations: `union_scan_covers_world`,
`world_consequences_exact`, `world_constraints_exact`, `chunk_concatenation`
and `completed_round_exact`. Concrete Rust traversal, packed storage and WGSL
refinement remain outside those theorems' established correspondence.

## Follow a candidate-dependent join

```asp
{{#include ../../../examples/lazy-reachability.lp}}
```

One answer set contains `blocked` and the two edge facts. The other contains
`start(1)`, `reach(1)`, `reach(2)`, `reach(3)` and the edge facts. In the relational
schedule above, when the seed selects `blocked`, no `start` or `reach` consequence is derived, so
the positive reachability joins have no matching rows. A seed selecting
`start(1)` enables those consequences over successive rounds.

That check begins with an empty derived interpretation, not with all seed atoms
asserted as facts. A complete no-growth round establishes closure; constraints
and agreement with the seed still have to hold. This explains the mathematical
schedule, not a speedup, a memory reduction or the route of a current command.

From the repository root, compare the complete answer families directly:

```sh
zetesis examples/lazy-reachability.lp --grounder eager --backend cpu --models 0 --stats
zetesis examples/lazy-reachability.lp --grounder lazy --backend cpu --models 0 --stats
clingo examples/lazy-reachability.lp 0
```

Answer order is immaterial. Both answer sets above must be present, with all edge
facts retained. The lazy command exercises the current hybrid formula route,
not the former relational closure schedule. On an accessible Metal device, it can instead
use `--backend metal`; inspect its statistics to confirm the executed route.
This tiny example tests meaning and routing, not throughput. For broader
reproduction, the [validation guide](../reference/validation.md) links maintained
corpora, physical checks and matched comparison schedules, with their limits and
reported observations. A performance claim needs its exact program, configuration,
completion evidence and measurement population as well as an elapsed time.

## What clingo already avoids

gringo does not enumerate an indiscriminate Cartesian product of every variable
over every constant. In the inspected clingo 5.8.2 implementation, predicate
domains have bound and full indexes, with `NEW`, `OLD` and `ALL` generations.
Dependency components, an instantiation queue and generation updates limit
repeated grounding work. The instantiator uses binding dependencies when
backjumping through a join. Indexed joins, component analysis and delta-based
iteration are therefore not unique to zetesis. See the pinned
[domain indexes](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libgringo/gringo/domain.hh),
[grounding components](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libgringo/src/ground/program.cc)
and [instantiation](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libgringo/src/ground/instantiation.cc).

clingo also supports grounding selected parameterized program parts over several
application steps. Cleanup can use a solver's top-level assignment to simplify
grounding domains before later steps. That feedback differs from zetesis's
former candidate-specific source rounds. It is inaccurate to describe ordinary clingo
grounding as necessarily materializing every future step at startup; its
[pinned control API](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libclingo/clingo.h)
defines the separate operations.

## What zetesis retains

Eager formula grounding uses indexed possible-support relations and explicit
binding/value plans, then instantiates complete formulas. Possible support is
an upper bound on producible atoms, not truth in an answer set. Negative
conditions and correlated aggregate choices remain in the formulas checked
under the original and frozen interpretations.

Hybrid grounding retains the complete producer core and the source plans for
eligible constraints. Each core answer must pass those constraints before
scoring or publication. When preparation also defers terminal definitions,
host reconstruction follows the base constraint checks and precedes publication
of the full answer. The [hybrid constraint schedule](grounding.md#eager-and-lazy-execution)
describes that boundary and its retained state.

Candidate coverage remains a separate cost. Formula regions retain held, cut
and open atom decisions; sound subjects can narrow them before splitting.
Possible support alone does not establish truth in an answer set. Membership
still requires the original theory and its frozen reduct. The
[representation correspondence](../lean/correspondence.md#ownership-and-execution-correspondence)
separates the represented state from its semantic laws.

## Hardware, memory, and comparisons

Parsing, source ownership and source joins run on the host.
The formula GPU path checks a retained theory: the complete eager theory or the
producer core of hybrid grounding. Tight cores use complete support checking;
general cores retain exact CPU completion of unresolved device results. Hybrid
answers then pass every streamed constraint on the host before scoring or
publication. This is not full GPU grounding.

Avoiding a rule vector can save storage. Repeated joins, masks, demanded
catalogs, concurrent workspaces and transfers also cost memory and time. A useful
comparison therefore records eager grounding and solving separately, interleaved
lazy work, actual device work, complete outcomes and resource limits. Logical
payload counters cannot stand in for measured process memory. Timeouts,
refusals and incomplete searches remain visible instead of disappearing from
the comparison population.

Source admission, `PreparedFormula::ground()` and already-admitted checking are
independent library boundaries. ASPIF import/export and a drop-in gringo/clasp
interchange interface remain unimplemented. The architectural separation makes
those consumers possible to design; it does not supply their contracts today.
