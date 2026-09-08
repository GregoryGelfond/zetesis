# Dependent aggregate producers and binary replacement

The pinned Lean library passes a clean build, strict compilation of all
**66 semantic modules**, and a complete **772-declaration axiom audit**. This
checkpoint adds seven `AggregateDependencies` and two `BinaryWatch` laws.
All 763 prior declaration names, source locations and transitive axiom sets are
unchanged; all 64 prior semantic module sources retain their recorded bytes.

[commands.json](commands.json) records the exact commands, working directories,
UTC start times, elapsed times and exits. The toolchain is Lean 4.33.1,
commit `819816b2e0a3bf405af45ae5c7af2491d8f5bee6`. `lake clean` precedes the
build. Every semantic module and `Audit.lean` is checked with
`-DautoImplicit=false` and `-DwarningAsError=true`. The full audit contains only
the established standard axioms `propext`, `Quot.sound` and `Classical.choice`,
or no axioms. No project axiom, proof hole or native-evaluation proof shortcut
was introduced.

The preceding manifest, theorem index, umbrella, audit source/output, README
and reading guide are preserved as `*.before.*`. The
[source continuity record](prior-source-continuity.json) and
[declaration comparison](continuity-check.json) distinguish preserved results
from the nine additions. Current `axiom-audit.txt` equals the retained
[audit.log](audit.log). The read-only proof-record checker, its regression tests
and the repository proof gate are recorded separately in
[check-commands.json](check-commands.json). Record consistency does not replace
Lean kernel checking or establish trustworthy execution by itself.

The [initial record check](initial-record-check.log) detected that the refreshed
manifest still held the prior hash of the current reading guide. The
[initial manifest](verification.initial.json) and
[command](initial-check-command.json) retain that failed consistency check.
Refreshing the current guide hash corrected the metadata; no Lean source or
kernel result changed, and the previous guide remains in `guide.before.md`.

`AggregateDependencies` composes complete candidate families indexed by their
own predecessor rows. Equal child values preserve their parent association.
A signed-sum specialization uses supplied complete full-tuple coverage.
Original and arbitrary frozen M/J rows retain prior activation/equalities and
the dependent aggregate equality. Total value functions and complete carriers
remain premises. Rust dependency extraction, safety, scheduling, joins, caches,
cursor resets, actual support coverage, machine arithmetic and resource
completion remain unproved implementation obligations.

`BinaryWatch` proves that two distinct valid positions exhaust a binary clause,
so a search excluding both watched positions finds no replacement under any
availability predicate or inspection order. The Rust implementation must
separately establish those watch invariants and preserve unit/conflict decisions.
Watch registries, propagation and candidate order, work charging, allocation,
CNF encoding and reduct construction are not verified by these two laws.

The original theory and its frozen reduct remain the permanent acceptance
foundation. These additions strengthen the mathematical library supporting
solver verification. They do not formally verify the Rust or GPU executable,
certify physical hardware, or establish a performance result.
