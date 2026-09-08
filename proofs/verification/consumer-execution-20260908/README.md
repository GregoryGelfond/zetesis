# Completed outer values and reusable join frames

The pinned Lean library passes a clean build, strict compilation of all
**64 semantic modules**, and a complete **763-declaration axiom audit**. This
checkpoint adds four `OuterNegativeConsumers`, five `OuterRanges` and six
`JoinFrames` laws. All 748 prior declaration names, source locations and
transitive axiom sets are unchanged; all 61 prior semantic module sources
retain their recorded bytes. The compact-trail experiment was declined and
contributes no registered module or theorem.

[commands.json](commands.json) records exact commands, working directories,
UTC start times, elapsed times and exits. The toolchain is Lean 4.33.1,
commit `819816b2e0a3bf405af45ae5c7af2491d8f5bee6`. `lake clean` precedes the
build, and every semantic module and `Audit.lean` is checked with
`-DautoImplicit=false` and `-DwarningAsError=true`. The full audit contains only
the established standard axioms `propext`, `Quot.sound` and `Classical.choice`,
or no axioms. No project axiom, proof hole or native-evaluation proof shortcut
was introduced.

The preceding manifest, theorem index, umbrella, audit source/output, README
and reading guide are preserved as `*.before.*`. The
[source continuity record](prior-source-continuity.json) and
[declaration comparison](continuity-check.json) separate preserved results from
the 15 additions. Current `axiom-audit.txt` equals the retained [audit.log](audit.log).
The read-only record checker and its 15 regression tests remain distinct from
Lean kernel checking; their results are retained in
[check-commands.json](check-commands.json).

`OuterNegativeConsumers` retains original aggregate equality and activation
while default and double negation read the frozen candidate. Negative projection
quantifies over the complete supplied witness family. These laws do not prove
Rust admission, safety, projection coverage, argument evaluation or scheduling.

`OuterRanges` preserves each supplied outer value's inclusive mathematical
integer range, complete concatenated expansions and original/frozen clause
association. Complete outer carriers and total endpoint functions remain
premises. Source support coverage, machine arithmetic, cursor state and
resource limits are unproved implementation obligations.

`JoinFrames` gives a finite predicate-store schedule: reset the root and
overwrite a child before using it as the next parent. Complete current-prefix
membership is independent of previously retained frame contents. A stale-root
counterexample exposes the required reset. Array bounds, machine words,
world-tail masks, allocation lifetime/budgets, Rust traversal and WGSL/device
correspondence remain unproved.

The original theory and its frozen reduct remain the permanent acceptance
foundation. This is a strengthened mathematical library for solver verification,
not a claim that the executable Rust/GPU solver is formally verified. The local
macOS qualification policy supplies no hosted Linux execution evidence.
