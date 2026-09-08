# Measured aggregate heads

Six new laws in `HeadMeasures` separate positive-head permission from an activated
numeric constraint. Permission retains the full original and frozen eligibility
formula. An activated bound inspects only the original candidate, so original
numeric agreement suffices for that constraint. The complete group then preserves
original/frozen truth and stable membership in any surrounding theory.

The pinned Lean 4.33.1 clean build, strict check of every semantic module and
complete axiom audit pass for 794 laws across 69 modules. All 788 prior theorem
names, source locations and transitive axiom sets remain unchanged; the prior
68 semantic module sources are byte-identical. The continuity record checks
those identities. Only the recorded standard Lean logical axioms occur.

The head carrier and its eligibility remain fixed through the numeric change;
a zero numeric contribution does not authorize dropping a head's permission.
Numeric compilation, signed normalization, complete tuple/atom and eligibility
correspondence, finite-width arithmetic, source semantics and Rust/WGSL refinement
remain unproved. The module does not resolve negative `#sum+` head behavior.
These laws are mathematical composition results, not concrete compiler verification.

Independent review reconciled the definitions with the actual permission and
activated-constraint lowering, and found no semantic blocker. Its wording
correction distinguishes sufficient equivalence from a necessity claim.
`commands.json` retains the clean build, per-module strict audit and the final
module/build/audit after that documentation-only correction. Final audit bytes
are identical. `module-initial.log.gz` preserves the first ambiguous `Neg`-name
compile failure; explicitly qualifying `Ferraris.Neg` resolved it.

`check-commands.json` retains record-regression and proof-gate checks. The main
verification record hashes these logs and preserves the previous complete record.
Task-local orchestration was not added as a new repository tool or dependency.

The initial record consistency check rejected two current audit entries after the
review rerun. `record-initial.json` and `initial-record-check.log` retain that
failure. The current command list selects the final audit; both actual audit runs
remain in the historical command log, with identical output. No gate was weakened.
