# Estate standards alignment — 7 September 2026

The engineering standard is useful here as a discipline for representing knowledge
explicitly and making claims that the implementation and evidence support. The
review uses the engineering principles and technical baseline at koinon revision
`074f2f0ab2bf129c03c19e5998bdf3ed0989e577`. The applicable expectations are stated
completely in this repository's [contribution guide](../../CONTRIBUTING.md).

Zetesis retains its existing CI and review process. The technical instrument
inventory identifies candidates for future assurance work; individual tools and
test runners require a separate technical assessment.

## What the review changed

| Finding before the review | Correction | Evidence of improvement |
| --- | --- | --- |
| `StageTimings::is_complete` treated the presence of unattributed time as sufficient. Public duration edits could make a contradictory partition report complete. | The predicate rechecks the exact checked sum of current measured and unattributed durations, including overflow markers. It remains usable in const contexts and specifies arithmetic consistency separately from provenance. | Three public regressions failed against the old predicate. The corrected predicate passes them, existing recorder tests and explicit overflow cases. |
| A JSON test claimed closure/formula coverage, but its multi-element choice made the automatic route fall back to formulas. | Separate singleton choices with explicit closure/countermodel selection; assert actual execution evidence and the exact four full interpretations. | Substituting the formula route for the closure case now fails the expected assertion. |
| An interrupted-optimization test described retained incumbent evidence but checked only status. | Assert verified/published counts, full retained model identity, matching typed/model/summary cost, scored models and tied models. | Erasing the serialized incumbent now fails the expected assertion. The test permits either valid first candidate, avoiding a dependency on search order. |
| Driver documentation promised publication as models were found and described cancellation too broadly as an interrupted report; `--models` help promised optimization always exhausts search. | Describe optimization retention and distinguish search interruption from view/publication failure. | The search implementation is unchanged; public documentation and generated help now match the actual outcome paths. |
| The bounded model view and telemetry primitives lacked explicit cost profiles and some local invariants. | Document ownership, fixed recorder costs, observation evaluation costs, selection comparisons, cursor work/space and decreasing traversal measures. | The implementation is unchanged; its costs and correctness argument can be assessed without reconstructing them from control flow. |
| Recent JSON, telemetry and model-view tests combined independently falsifiable claims; a private output operation was named `model`. | Separate the test propositions, preserve their assertions and name the effectful operation `write_model_record`. | Assertion mappings retain the prior cases; the two selected JSON controls still fail their intended assertions after decomposition. |
| The dated library audit still presented delivered JSON/timing work as prospective. | Add an explicit implementation follow-up while preserving the original design and remaining API/ASPIF work. | Current readers can distinguish implemented views from unfinished orchestration. |

The runtime correction concerns timing evidence, not answer-set membership,
candidate generation, objective ordering or GPU execution. Its loop maintains
`elapsed = unattributed + sum(processed stage durations)` with checked arithmetic.
Each iteration consumes one of four fixed slots. Successful termination therefore
establishes an exact arithmetic partition; unavailable data, overflow or a
mismatched driver duration cannot establish one. It does not prove who produced
the data or that solving completed successfully.

These are concrete improvements in correctness and assertion strength. Clearer
contracts and explicit invariants improve reviewability, but no controlled human
comprehension study or software-reliability measurement was performed. Neither
a style change nor a passing coverage number establishes mission-critical fitness.

The two intentional test failures are selected negative controls. They demonstrate
that these particular defects are detected; they are not a systematic mutation
campaign. The pre-fix timing failures, corrected tests, controls and validation
records are retained in [the alignment evidence](../verification/estate-alignment-20260907/README.md).

## Naming as representation

The contribution guide now states the naming principles explicitly. Domain names
identify concepts; documentation carries their full contract. Meaningful steps and
constants need names when those names add knowledge. Absence must not masquerade
as an ordinary value. A difficult name prompts a decomposition review.

The bounded test review separated the recent suites into individually reported
propositions: JSON 20 to 35, telemetry 10 to 18, and model views 4 to 14. The
[inventory and assertion mappings](../verification/estate-alignment-20260907/naming/README.md)
record what moved and which relational invariants remain together. More tests do
not imply proportionally more coverage. The practical improvement is that a
failure identifies a narrower claim, with setup shared through named fixtures.
The human-output test additionally checks the actual parsed default, instead of
assigning the desired value before testing it.

Fifty characters is a review prompt, not a ceiling. The fixed-baseline lexical
inventory flags 831 of 932 test declarations for reading on length alone; it does
not establish 831 defects or completion of a repository-wide naming audit. Older
suites need review by semantic area. Public vocabulary issues also remain:
`Model::new` constructs a canonical interpretation without establishing modelhood,
and `OutputSelection` selects the atom channel rather than all output. Those
boundaries should be refined deliberately with the library outcome, preserving
compatibility and requiring evidence for any proof-bearing result type.

## What the cost review exposed

The model view borrows a full model, its observation selection and an already
computed score; it owns evaluated observation terms. This preserves identity,
display and objective distinctions without copying the interpretation or rescoring
it. Encoding still pays for full output and selection: with A atoms and S selected
signatures, selection can require A×S predicate comparisons, including name costs.
Observation evaluation has its own bounded join cost.

The explicit symbol cursor bounds traversal depth and keeps its frame invariant
visible. Its `try_reserve_exact(1)` frame growth can cause quadratic cumulative
copying in the depth of a term. Retained encoder workspace remains proportional
to output bytes and depth. The documented cost is therefore not a blanket
linear-time claim. Bounded geometric frame reservation is a possible later
improvement, requiring the same refusal, allocation and traversal contracts.

The recorder keeps four fixed stage slots and one parent per active guard. Its
operations and integrity check have fixed cost; live guard space grows with
nesting depth. Disabled recording avoids clock reads and dynamic allocations,
which is weaker and more accurate than claiming no execution overhead.

## Current architectural and assurance position

| Objective | Implemented and demonstrated | Remaining boundary |
| --- | --- | --- |
| Independent ASP solver | Native Rust candidate generation and exact reduct checking; clingo is an external qualification oracle. Checked class certificates justify specialized membership where applicable. | Further language coverage and whole-architecture optimization. |
| Admitted language compatibility | All 94 original non-clingcon corpus contracts pass. Of 24 selected upstream fixtures, 21 pass complete model comparison and three retain explicit admission refusals. | This selected set is not a language-wide percentage. Broader binding/construction, heads/directives, undefined arithmetic and documented extrema edge cases remain. |
| GPU and parallel execution | wgpu/Metal reduct propagation, exact CPU residual completion, Rayon paths and previous physical M4 Pro qualification. | Execution is hybrid. General lazy formula grounding, persistent device execution and a demonstrated broad GPU advantage remain research/engineering work. |
| Library first | Reusable semantic kernels, domain analysis, typed model/observation views and independent telemetry. | Ordinary orchestration still depends on CLI options/writers. Successful public `Report` omits verified-model count even though internal progress/JSON and failure reports retain it. A shared typed outcome and model stream should close that representational gap. |
| Interchange and extension | Native semantic representations and in-repo ASPIF/theory/Rust-function designs. | Standalone ground-artifact interchange, ASPIF import/export and custom theory/function integration are not implemented by this checkpoint. |
| Mathematical library | 613 audited Lean theorems, pinned complete builds, explicit scope, one structured proof pilot and reading guides. | No Lean-to-Rust, source-lowering or shader refinement has been established. Proof count is not a substitute for those obligations. |
| Engineering assurance | Rust formatting/pedantic lint/strict docs, semantic/property tests, independent workspace/CPU CLI coverage floors, external parity, bounded failure tests and reproducible measurements. | Systematic mutation, input fuzzing, managed linted/typed Python tooling, operational reliability and wider platform qualification need further work. |

The baseline before alignment is `419ab3e`, with all four CI jobs green and the
installed library-view/timing release qualified on CPU. The alignment is a bounded
correction over that baseline, not a claim that every historical component has
been reviewed against every expectation. Exact final check identities and installed
binary associations belong to the linked evidence record.

## Next useful work

A shared library outcome should preserve verification, successful publication,
coverage, interruption and objective evidence for every consumer. This gives the
next orchestration extraction a precise representation to compose, including
already-admitted input and future ground-only/ASPIF doors. It should be tested
through both Rust and CLI consumers before additional abstraction is introduced.

Assurance work should target weak evidence first: systematic mutation around
outcome/coverage transitions and bounded fuzzing of source/view boundaries. Managed
Python lint/type checks would strengthen the independent comparison tools. These
are separate obligations, with explicit scope and retained failures.

Language compatibility and optimization remain active objectives. The existing
performance records identify eager grounding as a substantial SEND+MORE cost;
that is a concrete basis for a general binding/arithmetic optimization rather
than an inference that fewer reduct queries must make the whole solver faster.
