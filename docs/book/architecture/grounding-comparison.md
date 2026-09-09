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
| zetesis eager relational | A bounded complete graph is compiled explicitly from relational templates | The retained ground instances and graph identity |
| zetesis lazy relational | Templates remain available; source joins generate instances during frozen-gate positive inference | Final closure coverage and constraints, plus separately complete candidate enumeration |

These routes do not have equal language coverage. The
[language reference](../reference/language.md) identifies the current profiles.
General lazy formula grounding, including arbitrary bounded choice groups, is
not yet implemented.

## Follow a candidate-dependent join

```asp
edge(1,2). edge(2,3).
start(1) :- not blocked.
blocked :- not start(1).
reach(X) :- start(X).
reach(Y) :- reach(X), edge(X,Y).
```

One answer set contains `blocked` and the two edge facts. The other contains
`start(1)`, `reach(1)`, `reach(2)`, `reach(3)` and the edge facts. In a lazy check
whose seed selects `blocked`, no `start` or `reach` consequence is derived, so
the positive reachability joins have no matching rows. A seed selecting
`start(1)` enables those consequences over successive rounds.

The check begins with an empty derived interpretation, not with all seed atoms
asserted as facts. A completed no-growth round establishes closure; constraints
and agreement with the seed still have to hold. This example explains a work
schedule. It does not, by itself, establish a speedup or a memory reduction.

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
candidate-specific source rounds. It is inaccurate to describe ordinary clingo
grounding as necessarily materializing every future step at startup; its
[pinned control API](https://github.com/potassco/clingo/blob/a99ffb2a58293c68b28fcc283a1d1c9ccad900fe/libclingo/clingo.h)
defines the separate operations.

## What zetesis retains

Eager formula grounding uses indexed possible-support relations and explicit
binding/value plans, then instantiates complete formulas. Possible support is
an upper bound on producible atoms, not truth in an answer set. Negative
conditions and correlated aggregate choices remain in the formulas checked
under the original and frozen interpretations.

The lazy source scanner retains relation indexes, join frames and bounded
instances rather than a complete ground-rule vector. Shared rounds may scan
the union of candidate worlds; individual-world checks prevent cross-world
combinations from becoming facts. Optional membership masks prune only prefixes
with no witness in the current snapshots. The ordinary shared CLI route uses
union scanning; the world-mask policy is an explicit library choice.

Candidate coverage is a separate potential cost. `Candidates` begins with an
empty seed without expanding the carrier, but complete enumeration must still
cover the gate carrier. Its predicate/domain representation can contain many
combinations, and seed enumeration is exponential in the number of gate atoms.
A small demanded catalog during one check does not establish that other
candidate choices are irrelevant.

## Hardware, memory, and comparisons

Parsing, source ownership and source joins run on the host. In the relational
lazy device path, Metal computes per-world positive consequences and frozen-gate
tests from bounded source chunks; it is not replaying a CPU-completed closure.
The general formula GPU path consumes an eager theory and retains exact CPU
completion of residual membership work. Neither path is full GPU grounding.

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
