# Working on zetesis

Read [CONTRIBUTING.md](CONTRIBUTING.md) before making changes. It is the
authoritative contributor guide; this file is a concise entry point for coding
agents. Apply the same standards to implementation, tests, proofs and tooling.

## Find the relevant boundary

- Start with the [guided tour](docs/book/architecture/tour.md) and
  [semantic definitions](docs/book/architecture/semantics.md).
- Use the [library map](docs/book/rust/libraries.md) to locate the capability
  being changed. Keep reusable behavior available through typed library APIs.
- Check the [language reference](docs/book/reference/language.md) and
  [coverage checklist](docs/book/reference/language-coverage.md) before changing
  admission. Successful parsing does not establish solver support.
- Read the [proof boundary](proofs/README.md),
  [proof style](proofs/STYLE.md) and
  [implementation correspondence](docs/book/lean/correspondence.md) when a
  semantic invariant or representation changes.

## Preserve the semantic foundation

- The reduct defines answer-set membership. For normal rules, require the
  candidate to equal the reduct's least consequence set and satisfy its
  constraints. For general formulas, check
  original satisfaction and the absence of a proper-subset model of the frozen
  reduct. Specialized algorithms need an explicit preservation argument.
- Use the logic programmer's vocabulary at semantic boundaries: program,
  interpretation, satisfaction, reduct and `AnswerSet`. `WorldView` denotes
  the complete family of a program's answer sets. A candidate has not yet
  established membership; incomplete enumeration has not established that family.
- Keep semantic operations distinct from execution primitives such as joins,
  masks, reductions and fixed-point rounds. Eager or lazy grounding, Rayon and
  wgpu are execution choices that must preserve the semantic contract.
- Reuse themelios for source representation and analysis. Keep `themelios`
  lowercase, preserve provenance and typed logical values, and retain the
  reviewed dependency pin unless the change includes a dependency review.
- Keep clingo an external comparison oracle. Correct answer sets and observable
  contracts govern compatibility; clingo's internals do not define our algorithm.

## Make the correctness argument readable

- Design library operations and reusable orchestration first. Keep the CLI to
  argument mapping, source loading, presentation and exit handling. Separate
  pure transformations from I/O and execution effects.
- Give names to domain concepts and meaningful steps. State preconditions,
  invariants, termination bounds, failure semantics and costs where they apply.
  Represent absence and incomplete results explicitly.
- Resolve rustfmt, pedantic Clippy and documentation diagnostics in the code.
  Do not suppress `dead_code`, `unused` or `warnings`, manufacture uses or widen
  visibility to evade a check. Follow the contributor guide for narrowly
  justified foreign-interface lint exceptions.
- A test name states one proposition. Split independent claims; put rationale
  in comments. Fifty characters is a review cue, not a length limit. Assertions
  must detect a violation of the claimed behavior, including the execution route
  when the test claims to exercise one.
- Preserve resource accounting and failure boundaries. Cancellation, exhausted
  resources and device or writer failures must not become claims of
  unsatisfiability, exhaustive enumeration or proved optimality.

## Validate the affected contract

Use the maintained commands from the repository root. Follow the
[tool setup](docs/book/reference/validation.md#prepare-verification-tools) and
[contributor guide](CONTRIBUTING.md#verification-and-review) for prerequisites,
coverage populations and evidence requirements.

| Command | Scope |
| --- | --- |
| `scripts/check.sh portable` | Rust tests, formatting, strict lint and docs, benchmark correctness |
| `scripts/check.sh oracle` | External clingo comparisons |
| `scripts/check.sh coverage` | Independent workspace and CPU-only solver/CLI coverage floors |
| `scripts/check.sh coverage --metal` | Coverage with the specified physical Metal tests |
| `scripts/check.sh hardware` | Physical qualification of the host's device backend: the reviewed exact device tests, answer sets equal to the CPU's; `--metal` or `--vulkan` names the backend |
| `scripts/check.sh proofs` | Lean build, axiom audit and proof records |
| `scripts/check.sh book` | Manual build and checked examples |

Keep each baseline, candidate and mutation checkout's Cargo target directory
separate. Report the checks actually run and any remaining qualification.
CPU checks do not qualify a GPU; existing Lean laws do not establish complete
Rust or shader correctness. Performance claims need reproducible measurements
with source, binary, backend and workload identities. Never weaken a gate to
accommodate a change.

Follow the contributor guide's [build storage policy](CONTRIBUTING.md#build-storage).
Assign build owners, check disk space before large builds, and retire completed
caches after preserving the specific evidence and comparison artifacts still
needed. Do not retain entire superseded target trees by default.

## Keep public documentation current

Update affected manual explanations, runnable examples, source references and
Lean statements with the implementation. Document supported behavior and limits
together. Keep durable design arguments and reproducible fixtures in the
repository; keep development diaries, private continuation notes, machine-local
paths and temporary session instructions outside it.
