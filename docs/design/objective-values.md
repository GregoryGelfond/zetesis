# Resolved objective weights

Closed nonnumeric weights now use the same contract as nonnumeric values reached
through a bound variable: they contribute no numeric key and supply no numeric
priority witness. This includes symbols, signed symbols, strings, closed
functions, closed tuples, `#inf` and `#sup`. Minimize, maximize and weak constraints
share that selection rule. Numeric zero still supplies a key and a priority slot.

The frontend admits the complete objective element before grounding decides
presence. An ignored weight cannot hide an unsafe tuple, unsupported condition,
dynamic priority, invalid sibling or exceeded source allowance. Source objective
declarations and original locations remain retained. The implementation changes
weight admission only; extrema used as weights are ignored logical values and do
not relax the separate arithmetic endpoint policy.

`ObjectiveTemplate` continues to own the original logical term. After complete
possible support, `objective_active` asks whether an admitted template has a
numeric witness. Only those templates enter `ObjectiveProgram`. Exact evaluation
later selects numeric weights in a verified interpretation, normalizes direction,
and coalesces the complete priority/weight/tuple key. Those two selection stages
have different input relations; neither derives priority presence from the set
of accepted models. The original formula theory and its reduct remain unchanged.

No callback, process invocation or alternate objective evaluator is introduced.
The original work, substitutions, source-template and copied-value budgets apply
before publication. Failure returns no partially admitted objective program.

The literal qualification matrix crosses nine spellings, all three directives,
unconditional, optional, absent and false conditions, and same/different-priority
numeric controls. Independent expected records cover absent costs versus a
present numeric-zero priority, duplicate numeric keys and every full model of
two independent choices. Empty-program, strong-negation spelling, invalid sibling,
exact-limit and property controls supplement those cases. External qualification
retains the original source and complete clingo JSON separately from the native
checks; `--opt-mode=enum` records all models and is not an optimizing-session test.

`ObjectiveValues` establishes numeric selection, ignored-entry deletion,
priority presence, complete-key preservation, cost preservation and optimal-model
preservation for already-resolved finite entries. It does not verify the Rust
compiler, finite-width arithmetic or clingo's grounding carrier. Prepared
optimization sessions and physical backends require separate qualification.

Mixed-extrema presence remains governed by the separate
[presence contract](objective-presence.md). Accepting a literal nonnumeric weight
does not remove the variable-weight mixed-carrier guard, certify filters or
objective-relevant function heads, or implement dynamic priorities. Constructive
extensions of that carrier are a subsequent slice.
