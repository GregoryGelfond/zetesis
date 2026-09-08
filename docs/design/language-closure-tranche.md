# Language closure before dedicated optimization

The next tranche concentrates on closing the remaining intended source-language
and observable-semantic gaps. The following tranche concentrates on optimization,
stronger Lean correspondence, API cleanup and user friendliness. This sequencing follows
the aggregate-primitives qualification.

The reduct remains the acceptance foundation. Language coverage should expose
the right shared operations rather than accumulate syntax-specific workarounds.
Keep necessary semantic proofs, resource controls, code clarity and regression
gates active throughout language work; they are not postponed until optimization.

## Make closure an explicit contract

Use the [compatibility matrix](../verification/clingo-compatibility.md) as an
obligation ledger. It is not a completed-compatibility claim. For each gap, retain
an original source, its independent reference behavior, the precise refusal or
incorrect behavior, the owning representation/consumer and its completion tests.
Include interactions between features, not only one positive example per token.
Passing the 94-case corpus is necessary but does not close this ledger.

Separate these outcomes:

| Category | Closure treatment |
|---|---|
| Approved exclusions | Theory atoms/terms, Python/Lua scripting, `#heuristic` and `#edge` retain explicit refusals. They do not count as passing solves. |
| themelios frontend boundaries | Preserve upstream parse, raising and evaluation provenance. Do not attribute a zetesis guard to themelios. |
| Internal language gaps | Close the implementation or correspondence obligation; current refusal is not evidence of a KR error or unsafe source. |
| Resource stops | Report incomplete work, with exact retained accounting. They are neither UNSAT proofs nor syntax exclusions. |
| Execution capabilities | Lazy/eager eligibility and hardware capabilities remain separate from source-language acceptance. |
| Explicit release decisions | Record any agreed deferral without changing the larger compatibility target or silently shrinking the release profile. |

In particular, [numeric semantics](numeric-semantics.md) distinguishes the current
internal extrema-endpoint guard from the separate upstream raising limitation
for literal `-2147483648`. General checked `i32` overflow/undefined-expression
behavior still needs its stated correspondence. A wider native accumulator does
not settle those source-language questions.

`#project`, named/parameterized program parts and `#external` are not approved
exclusions. Their observation, activation and external-truth contracts remain
obligations. Rust `@` functions are an intended first-class themelios extension;
their release placement must be stated explicitly, independently of excluded
Python/Lua scripting. Source-language closure does not imply libclingo ABI
compatibility, complete multi-shot execution or every backend/grounder pairing.

## Shared foundations

Shared foundations precede dependent consumers. The following bounded language
slices retain their own qualification obligations; this list does not exhaust
the language-closure ledger.

| Capability | First deliverable | Shared boundary |
|---|---|---|
| Head/group foundations | General complete-tuple activity for count heads, including both tuple/head alias directions | Separate atom permission from OR-coalesced tuple activity and aggregate value. Preserve existing measured-head profiles. CountPlan must retain its stronger premises or decline these groups without refusing ordinary solving. |
| Objective dependencies | Ordinary acyclic pass-through producers of aggregate results, then qualified consumers | Preserve realized values, global contribution keys, priority-slot presence, absent versus present-zero costs and all optimum ties. More complicated producers follow the established presence contract. |
| Independent language slices | Binding/conditional/pool contexts or directive contracts whose shared foundations are ready | Select explicit gaps from the ledger with original clingo cases. Preserve the established binding and group contracts. |
| Feature composition and semantics | Public APIs, ledger reconciliation, proof composition, cross-feature tests and release gates | Keep original program identity, exact reduct acceptance, source ownership and complete outcome accounting explicit. |

The first head slice is motivated by currently refused examples such as
`1#count{1:a;2:a}1.` and `1#count{1:a;1:b}1.`. Removing the existing bijection
check is insufficient: the representation must distinguish tuples from atoms.
Extend `CountHeads` and compose with `HeadMeasures`; neither existing law already
proves this generalized compiler.

The first objective slice includes
`n(N):-N=#count{}.p(X):-n(X).#minimize{X@7:p(X)}.`. Its dependency is simple, but
reported objective presence still matters. The [presence contract](objective-presence.md)
records cases where accepted-model inspection alone cannot reconstruct that
layout. Negative/disjunctive/conditional producers and dynamic priorities remain
visible follow-on obligations.

Negative `#sum+` heads need their recorded semantic discrepancy resolved, rather
than simply deleting a weight check. Full ordered-value head extrema, broader
conditional disjuncts, mixed evaluated/witness arguments, finite pool contexts
and remaining generator profiles likewise stay in the ledger until qualified.

## Record connections exposed by language support

Each slice should identify reusable concepts and the optimization opportunities
they make possible, with prerequisites and evidence. This is part of deliberate
representation design, not a competing optimization implementation track.

Examples include complete tuple activity shared by native reductions, explicit
producer/consumer dependencies shared by relational execution, and program-part
or external-state boundaries useful for later incremental/resident processing.
Similar-looking constructs can still have different semantics: objective-slot
presence is not aggregate truth, and head permission is not tuple contribution.
Share only operations with a common established contract.

The following tranche can then investigate native aggregate integration into
ordinary solving, certified simplification before joins, bounded numeric-column
execution, alternative truth layouts and their Lean correspondences. The
[foundation survey](foundation-optimization-opportunities.md) records these
proposals and the existing indexing/filtering baseline. New language support
can change their applicability; do not commit to an optimization before that
representation and its measurements are ready.

The [grounder research report](../research/grounder-optimization.md) develops
eager/lazy join, column, incremental and GPU-residency opportunities, with
semantic obligations and bounded experiment protocols for those later passes.

## Completion criteria

Every obligation in the declared release language profile must have an admitted
contract and passing independent semantic evidence, or an explicit agreed scope
decision. No intended gap becomes closed merely by being documented as refused.
Replay the in-repo corpus, selected upstream cases and feature-interaction
regressions; expand curated external coverage where it reveals an untested
construct. Preserve full-model, display/projection, cost and completion scopes.

Keep both independent 91% line-coverage floors, pedantic Clippy, formatting,
warning-denied documentation and appropriate physical qualification. Maintain
the Lean library alongside each semantic extension and state what concrete
correspondence remains unproved. Retain performance regression checks without
starting a separate broad optimization campaign during language closure.

Completion requires a reviewed language ledger, current README/API guides, a
qualified checkpoint and a bounded optimization plan. The resulting
claim is closure of that explicit source-language profile, not all clingo APIs,
unbounded resource guarantees or a fully formally verified solver.

## Library integration after language closure

The dedicated API-cleanup rounds should make zetesis an idiomatic estate library
and an eventual backend for `themelios-solve`. Review the interfaces against the
then-current themelios program and solving contracts, with keryx and morphe as
read-only examples of estate conventions. Preserve the logic programmer's
vocabulary and reuse themelios abstractions wherever their semantics fit. The
future solving API is still developing; this is an integration criterion, not a
claim that a backend adapter already exists.

Exercise grounding alone, solving an admitted program, and their composition
through small in-process Rust clients. Program/source ownership, prepared
representations, candidate streams and reduct outcomes should have explicit
contracts. Include diagnostics, resource limits, cancellation, completion,
statistics and model observations in the review. CLI rendering and serialization
remain consumers of typed library results, and hardware resources and scheduling
policy should be supplied through clear boundaries.

The desired integration has a small adapter with no duplicate grounding or
acceptance logic. Source admission, native ground-program views and interchange
views must state their information loss and supported semantics. Keep future
Rust-function and theory-extension boundaries in view without adding those
implementations to this cleanup round. The
[library-first design](library-first-20260907.md) remains the architectural basis;
concrete alignment should follow the actual `themelios-solve` API when available.
