# zetesis beneath themelios-solve

themelios owns the estate's user-facing Rust solve/session/extension vocabulary.
zetesis is an experimental native engine that could implement that contract.
It should not establish a competing public session or callback framework.
Its current CLI and internal crates are a prototype integration surface, not an
implemented themelios-solve adapter.

The design reference is themelios `docs/design/solve.md`, dated
2026-09-03, inspected at revision
`c4d4045dd0e248ce1922dfa6aa3bfa968b7c6d1d` with SHA-256
`c1a0360242ea123e24dc121678d685386b52a93f72fe25d65e07acda8eef8513`.
That document is explicitly pre-implementation. This inspection does not change
zetesis's dependency pin (`87c11a3f2b72b81a12fd53226941fdf95e7294d3`) or modify
themelios. Final adapter signatures follow the implemented shared contract.

## Boundary correspondence

| themelios design vocabulary | zetesis responsibility and present boundary |
| --- | --- |
| `Backend`, `Capabilities` | Provide an internal engine implementation with truthful per-operation capabilities; current CLI policies are not the future author-facing API |
| `Door::Ast`, `Door::Program` | Consume shared typed syntax/program values and preserve `Origin`; the current text/bundle frontend must gain a bounded public typed entry point before adapter conformance is claimed |
| `Door::Aspif`, typed ground sink | Keep solver ingestion separable from source candidate construction; no aspif adapter is implemented yet |
| `Solved`, `AnswerSet`, `Conclusion`, `Fault` | Return typed models, completion and errors; map current independent identities, partial coverage and resource refusal without parsing CLI prose |
| `Optimized`, `OptimizeRequest` | Expose proved optimum separately from incumbent trajectory and all optimal ties; exact reduct membership remains internal |
| `Scenario`, assumptions, consequences | Implement only when admitted by declared capabilities; no present support is inferred from ordinary enumeration |
| `ground_program()` observer | Offer an explicit complete materialization when supported, with provenance and cost; never label a partial lazy relation as a completed ground program |
| Shared session and `multi_shot` capability | Preserve program-side state in themelios and engine-side versions in zetesis; reuse requires the incremental-session invariants |
| `Function`, `GroundFault`, shared `Symbol` conversions | Support future Rust ground-time `@` callbacks through the shared extension contract |

The future bridge must not render an owned `Program` to text and reparse it.
The same typed program carries [input/output interface evidence](program-interface.md),
global [domain analysis](domain-analysis.md) and task-relative
[observation demand](demand-and-magic-sets.md) before construction or solving.
Source admission, relational candidate construction, formula lowering and exact
membership are separate mechanisms. A lazy solve must not require complete
ground-program materialization merely to satisfy an observer interface.
Conversely, a requested complete observer must either fulfill its contract or
return a typed refusal. The draft's observer capability and optional result
should be reconciled explicitly in conformance tests when the shared API lands.

The current scalar core is narrower than themelios's `Symbol` algebra. Adapter
conversion must preserve identity for admitted values and refuse unsupported
ones; compound/strong terms cannot be flattened into strings or silently lost.
Canonical atom order, model identity, hidden-model multiplicity, objective
priorities and source provenance must survive round trips. The estate's typed
result models own human and machine views; CLI output is one view only.

## Rust ground-time functions

Python/Lua scripting remains excluded. Rust `@` functions are a planned
first-class extension, distinct from embedded-language execution and theory
atoms. The current frontend still explicitly refuses `Term::External`.

The themelios draft registers a `Function` on a session, accepts shared typed
`Symbol` arguments, returns finite multiple symbols and reports a located
`GroundFault`. zetesis should consume that contract when it is implemented,
including the shared conversion rules and panic containment, rather than invent
another callback signature or numeric codec.

For zetesis's lazy generator, callback evaluation is a source construction step.
Returned values may enlarge the bounded possible carrier. A completed admitted
program version must fix their resulting facts/term expansions before those
results are relied on by a reduct proof. Callback failure, invalid values,
cancellation or exceeded output/work limits produce typed failure or incomplete
construction; they cannot mean an empty answer-set result.

Memoization and parallel invocation require the declared determinism and
thread-safety contracts. A function/context registration change invalidates
dependent cached results. An effectful callback needs an explicit execution and
snapshot contract; it cannot be treated as a pure mathematical function merely
because its Rust signature takes `&self`. Calls should be attributable to original
source sites and the exact program version used by the solve.

Rust callbacks initially execute on a Rust-capable host; supporting them does
not imply compiling arbitrary Rust libraries into WGSL or neuromorphic kernels.
Their bounded results can feed subsequent accelerated operators. Analysis and
backend planning must account for this placement and transfer cost.
The [Rust function execution contract](rust-functions.md) specifies versioned
results, admission, lazy coverage, limits and the future refinement obligations.

## Conformance before integration

Use the same typed programs through the clingo-backed themelios implementation
and a zetesis adapter, comparing complete outcomes and capability refusals.
Test macro/programmatic equivalence, conversion failures, original locations,
partial solves, hidden ties, cancellation and repeat calls. Keep external clingo
as an independent test oracle, never a production dependency of zetesis.
Incremental sessions and Rust functions need their own conformance campaigns;
the current 94-case single-shot corpus does not establish either capability.
