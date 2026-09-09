# Flat extrema and numeric priority presence

An unconditional unary assignment such as
`n(N):-N=#max{2:a;foo:b}.` has a bounded completed-presence certificate when its
tuple values are closed and each tuple condition is empty or one positive closed
atom. Every producer of a condition predicate must be a ground fact or an
unconditional unbounded ordinary choice with closed heads and empty element
conditions. Strong signs and complete ground arguments retain their identities.
A fact overrides an optional occurrence of the same atom.

The assignment predicate has exactly one producer. Relevant downstream consumers
may only be unique unary positive renamings, including finite chains and reordered
source statements. Unrelated consumers outside the objective dependency cone do
not affect this certificate. Existing preparation rejects unsupported negative,
disjunctive and conditional objective dependencies. Nonunary producers, outer
domain joins, variable tuple values, conjunctions, bounded choices, alternative
producers and nested reductions do not gain a mixed-carrier certificate. These
are zetesis implementation gaps.

Every possible numeric witness remains unless a required tuple dominates all
numbers. For a minimum, `#inf` has this property; for a maximum, symbols, strings,
functions, tuples and `#sup` have it. Optional conditions remain possible even
when they share atoms or constraints prevent their activation. In particular,
`{a}.n(N):-N=#max{2:a;foo:a}.#minimize{X@7:n(X)}.` retains priority 7 with zero
cost in both models. A realized numeric extremum is not required to witness the
completed grounding layout.

With `b.` and optional `a`, the first maximum always has a required `foo` tuple.
Its variable-weight observer has absent costs. The possible `n(2)` and empty
endpoint proposals remain in the original ground theory with their aggregate
equalities. Only completed numeric priority presence changes; acceptance still
checks the original Ferraris reduct.

The private `Presence` value borrows predicate identities from preparation.
`may_have_numeric_weight` excludes a witness only when the same unary generated
value is the template's weight. False means a proved exclusion; true still needs
the existing completed-support witness check. It cannot erase constant-weight
observers, zero at the same priority, unrelated numeric bindings in other body
atoms or another template's contribution. Evaluation retains the global complete
key, checked direction normalization and objective ordering.

Complete local joins first detect a mixed carrier. Classification inspects every
tuple and condition producer. Consumer transport uses a borrowed dependency cone
and finite scans; each changing round discovers a source predicate. Inspections
of tuples, predicates, rules, edges, ancestors and objective body atoms consume
formula work. Joins retain substitution and scalar-copy accounting. No alternate
support relation is stored.

`FormulaLimits::max_objective_presence_entries` bounds simultaneous logical
pointer slots across completed exclusions and temporary dependency/transport
sets. Duplicates consume work without another slot. Each insertion checks the
limit first; merging consumes a temporary entry before transferring it. The limit
counts entries, not allocator bytes or internal `BTreeSet` overhead. Default
16,384 matches the existing aggregate-plan row scale. Zero refuses a required
mixed plan and leaves homogeneous carriers unaffected. A stop returns located
`FormulaResource::ObjectivePresenceEntries` without a partial objective program.

Worst-case work includes a rule scan per transported predicate and a graph scan
per dependency-cone round, with membership inspections charged. Retained space
is linear in the entry ceiling. This language extension makes no performance claim.

Seven unchanged historical mixed-carrier examples now admit complete model/cost
parity; outer-domain and nested-reduction controls remain refused. Additional
originals cover correlation, constraints, signs, ground arguments, mandatory
duplicates, zero/shared priorities, unrelated weights, source permutations,
weak/maximize directions and inconsistency. Original theory identity checks every
original and frozen interpretation, including unrealized proposals. Properties
and exact limits supplement those cases; optimizing sessions and devices require
separate qualification.

`ExtremumPresence` proves witness exclusion, optional witness retention, carrier
monotonicity, transport and nonnumeric extremum selection under an explicit order
law. It does not verify the Rust recognizer, ASP term ordering or clingo's carrier.
External checks qualify the retained finite sources, not general gringo equivalence.
