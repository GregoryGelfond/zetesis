# Completed consumers, optional indices and world membership

The complete pinned Lean library passes a clean build, strict compilation of
all **61 semantic modules**, and a complete **748-declaration axiom audit**.
This integration adds six `ChoiceConsumers`, three `NegativeEligibility`, seven
`OptionalIndex` and nine `WorldMasks` laws. All 723 prior theorem names, source
locations and transitive axiom sets are unchanged; all 57 prior semantic module
sources retain their recorded bytes.

[commands.json](commands.json) records exact arguments, working directories,
UTC start times, elapsed times and exits. The toolchain is Lean 4.33.1, commit
`819816b2e0a3bf405af45ae5c7af2491d8f5bee6`. The build is preceded by `lake clean`;
individual module checks and `Audit.lean` use `-DautoImplicit=false` and
`-DwarningAsError=true`. The full audit contains only the established standard
logical axioms `propext`, `Quot.sound` and `Classical.choice`, or no axioms.
No project axiom, proof hole or native-evaluation shortcut was introduced.

The previous manifest, index, umbrella, audit source/output, README and reading
guide are preserved as `*.before.*`. [Source continuity](prior-source-continuity.json)
and the [declaration comparison](continuity-check.json) make the boundary between
previous and added laws explicit. Current `axiom-audit.txt` matches [audit.log](audit.log).
The record checker and its mutation regressions remain separate from kernel
checking; their command results are retained in [check-commands.json](check-commands.json).

`ChoiceConsumers` retains the original aggregate equality and whole-group
activation when a completed scalar filter selects a bounded choice group.
Complete proposal coverage, total evaluation and natural bounds remain premises.
`NegativeEligibility` specializes existing arbitrary eligibility formulas to
negation and double negation. Its counterexample prevents replacing a frozen
double-negative gate with positive reduct support. Rust source scheduling,
local safety, projection, signed arithmetic and complete tuple correspondence
remain unproved.

`OptionalIndex` proves representable positive-successor identities, optional
round trips, injectivity, absence and commuting link replacement. It does not
prove Rust niche layout, vector allocation, watch relocation, work charging or
search correctness. `WorldMasks` proves that prefix membership is conjunction,
empty prefixes have no enabled extension, and an exhausted masked scan covers
every world's enabled consequences and constraints. Cross-world conjunction and
stale-snapshot examples expose the coverage assumptions. Concrete row lookup,
packed bits, duplicate occurrence identity, snapshot refresh, resource bounds,
WGSL and physical execution remain implementation-refinement obligations.

The original theory and its frozen reduct remain the permanent acceptance
foundation. These additions strengthen the reusable mathematical foundation;
the theorem inventory is not a claim of a formally verified executable solver.
