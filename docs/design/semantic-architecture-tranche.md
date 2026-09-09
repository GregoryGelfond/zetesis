# Semantic architecture and library alignment

This is the next planned tranche after the qualified language-values and lint
checkpoint `944b06e`. It prioritizes semantic clarity, reusable library operations
and proof correspondence. It is a plan, not a claim that the proposed interfaces
or implementation proofs already exist. Remaining language extensions stay on
the backlog while this work establishes a clearer foundation.

## The semantic spine

An ASP reader should be able to follow the defining construction directly:

1. An interpretation satisfies a program or theory.
2. A candidate interpretation determines a reduct of the original subject.
3. The candidate is an answer set precisely when it is a minimal model of that
   reduct.

For a finite positive normal program, the headed rules have a least model,
computed by positive closure. Constraints are checked separately. This supports
the familiar characterization that the candidate equals the least closure of its
reduct and satisfies the constraints. Positive disjunction needs minimality:
`a | b.` has two incomparable minimal models, `{a}` and `{b}`. Neither a unique
least model nor a cheap general positive-program solver may be assumed.

For the Ferraris theory, the general contract is

```text
answer_set(P, M) iff
    satisfies(M, reduct(P, M)) and
    no J properly contained in M satisfies reduct(P, M).
```

The candidate `M` remains frozen when testing another interpretation `J`.
Replacing the subject with `reduct(P, J)` would ask a different question.
Satisfaction of the original theory by `M` is equivalent to satisfaction of its
own reduct; that equivalence justifies an original-model test in the checker.
Choice, nested negation and aggregate lowering retain their Ferraris contracts;
absence of a textual `not` is not a certificate that Horn closure is sufficient.

## Vocabulary and boundaries

Prefer `AnswerSet` in the domain-facing vocabulary. An unchecked value remains
an `Interpretation` or candidate. Naming must retain the distinctions between
least and minimal models, satisfaction and acceptance, and an unfinished check
and a completed negative result. Existing stable-model terminology in theorem
names may remain where it identifies the literature's equivalence law.

`WorldView` denotes the set of all of a program's answer sets. A partial stream
or collection must not silently stand in for that complete family. A lazy or
symbolic representation is possible, but its completeness and enumeration state
must be explicit. State the subject, assumptions and objective scope; an optimum
selection must not silently change the meaning of an unqualified collection.

| Layer | Reader's concepts | Obligation |
| --- | --- | --- |
| Program and results | Program, Rule, Atom, Interpretation, AnswerSet, WorldView, optimum, completion | Use the logical subject and typed answers; keep shown output a view. |
| Semantic operations | Satisfaction, Reduct, positive closure, minimality, candidate generation | Define mathematical contracts independently of scheduling and storage. |
| Execution | Bind, Filter, Gate, Project, joins, fixed-point rounds, masks, reductions, batches | Implement those contracts with bounded CPU/Rayon/GPU work and explicit incomplete outcomes. |

This is analogous to themelios's separation of concrete syntax from the logical
Program, rather than an assertion that syntax is an execution layer. Module
boundaries should establish the distinction first; a new crate needs its own
cohesive consumer and dependency justification. Avoid a mechanical rename or a
single generic operation that conceals mathematically different checks.

## Existing foundation and proposed work

The existing Rust Ferraris library exposes `models`, `models_reduct` and `check`.
Its bounded reference checker already freezes the candidate once inside a check.
However, a separate call to `models_reduct` allocates scratch and evaluates the
candidate again. Investigate a reusable, subject-bound frozen reduct view that
can answer several satisfaction queries without repeating that preparation.
Benchmark that client pattern before claiming a benefit to ordinary solving.
Identity, cancellation, allocation and work accounting remain part of the API.

The CPU oracle already computes positive closure from empty derived relations
and separates candidate seeds from completed outcomes. Make the closure result
and its constraint/seed checks easier to reuse and read without weakening source
coverage or accepting an unfinished fixed point. Lazy evaluation need not
materialize a complete ground reduct; its completed instances must still denote
the same semantic subject. A reduct view may be implicit, and execution may fuse
stages when a stated equivalence preserves the semantic boundary.

Lean already defines independent satisfaction and reduct operations in
`Ferraris.lean`, proves `stable_iff_minimal_reduct`, and proves the normal closure
characterization `stable_iff_gamma` in `Semantics.lean`. `FerrarisMask.lean` relates
frozen-mask evaluation to the explicit reduct. Add an explicit bridge between
the normalized rule semantics and the Ferraris translation used by Rust's
`normal::from_ground_program`, including constraints and candidate-bound gates.
Keep a theorem inventory that distinguishes semantic laws from unproved source
lowering, representation, Rust/WGSL and scheduler correspondence.

## themelios-solve alignment

The inspected themelios `docs/design/solve.md` is a draft design of
record, not an implemented dependency surface. Its small engine-free backend
contract, typed outcomes, session layer, extension seams and conformance suite
are the integration target. It proposes `AnswerSet`, streaming `answer_sets`,
exhaustion-gated `all_answer_sets`, and separate logical determination and search
conclusion. Align concepts and preserve these distinctions without claiming
that the draft methods are already callable in the pinned dependency.

Consume the Program through typed boundaries; do not render and reparse at the
integration seam. Keep compact native representations behind faithful symbol
and provenance mappings. A lazy ground-program observer must identify exactly
what it contains; an incomplete instance catalog cannot pose as a complete
eager ground program. Preserve zetesis's supported backend controls while
keeping hardware execution details below the prospective abstract solve seam.
No edits or dependency-pin changes to themelios are part of this tranche.

## Design boundaries and before/after evidence

| Area | Deliverable |
| --- | --- |
| Semantic API and clarity | An ASP-first entry point, naming map, subject-bound semantic values and cohesive module boundaries; compare the draft solve contract. |
| CPU composition | Reusable frozen-reduct satisfaction and positive-closure boundaries where measurements justify them; preserve bounded checks and lazy coverage. |
| GPU composition | Map the same contracts onto existing masks, batched satisfaction, closure and exact residual completion; preserve physical-work accounting and resident identity. |
| Proof composition | The normalized-rule/Ferraris bridge, scoped refinement obligations, coherent shared-interface contracts and regression qualification. |

Freeze the five currently qualified release binaries, configuration and source
identity before implementation. Establish scalar, Rayon and Metal measurements
over matched candidates and original sources; include eager/lazy routes only
where supported, and report refusals as refusals. Separate preparation, reduct
formation, satisfaction, closure, minimality and candidate generation where the
current instrumentation can measure them. Any new attribution must state
whether stages overlap or are fused. Record end-to-end time, peak resident
memory, allocations where measurable, device transfer and dispatch work, and
complete/partial outcomes. Use quiet paired measurements and distributions;
timing correctness tests does not establish a speedup.

Integrate small slices against frozen full answer-set identities, objective
absence and zero priorities, optimum ties, interruption and resource behavior.
Use independent original-source/clingo comparisons as well as old/new checks,
since two implementations can share a defect. Run the existing strict Rust,
documentation and proof gates, both independent 91% coverage floors, and fresh
physical qualification when compiled device-test inputs change. A clearer API
does not justify a semantic or unexplained performance regression. A measured
tradeoff must be explicit; no speedup or full executable verification is promised
by this architectural work.
