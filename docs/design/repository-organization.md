# Repository organization

zetesis is a Rust solver with a Lean proof library and WGSL device kernels.
Repository organization should make that identity evident. The implementation,
its semantic specifications, reusable validation tools and curated ASP fixtures
belong in the source repository. Full third-party implementations and accumulated
raw experiment output do not belong in its ordinary working tree.

This is an accepted cleanup direction, not a description of completed migration.
It complements the current language and grounder work without changing reduct
semantics, weakening qualification, or rewriting historical evidence.

## Current inventory

An inventory of tracked files at `39719a8b2a8c215d273fd0efdc40d9d8b429c69c`
found 11,014 files under `docs/`, occupying 1,088,479,861 uncompressed bytes.
Most are verification records. The tracked tree also contains 127 Python files,
three Objective-C probe copies, and three upstream C++ files. These are source
file counts, not runtime dependency counts. The C++ files preserve clingo fixture
provenance and are not linked into zetesis. Python is nevertheless a current
development dependency: portable checks, proof-record verification, coverage
checks and external comparison tooling use it.

The problem is substantive. Merely excluding those files from language statistics
would leave the checkout and development workflow unchanged.

## Intended layout

| Location | Responsibility |
| --- | --- |
| `crates/` | Solver libraries, the CLI, reusable validation libraries and Rust development commands. |
| `proofs/` | Current Lean modules, pinned toolchain, theorem catalog and current verification manifest. |
| `validation/` | Curated ASP programs and data, expected semantic outcomes, provenance and licenses. |
| `examples/` | Runnable documentation examples and the self-contained kr-domains regression collection with sealed provenance. |
| `docs/design/` | Current architecture, semantics, API contracts and decisions. |
| `docs/verification/` | Concise assessments and indexes identifying immutable evidence artifacts. |
| `experiments/` | Clearly scoped, reproducible experiments with named hypotheses and current relevance. |
| `scripts/` | Only small platform/bootstrap wrappers where a shell wrapper improves usability. |
| `target/` | Ignored local build output, captures, generated reports and temporary experiments. |

WGSL remains source code beside its Rust backend. Rayon is a Rust dependency;
neither its use nor wgpu requires an additional project language. An optional
platform probe is not a justification for shipping an otherwise unused native
source tool once its diagnostic purpose has been replaced.

## Curated external fixtures

Each imported case must be runnable without an upstream source checkout or a
C++ decoder. Retain the ASP source bytes, case identity, expected complete models
and objective semantics, and applicable license notices. A provenance manifest
records the upstream repository, immutable revision, source path and assertion
span, source hashes, and any extraction or adaptation. Adaptations must be named;
an adapted regression must not be described as an unchanged upstream case.

For the existing 24 clingo assertions, the current `cases.jsonl` already retains
decoded ASP, full-model references and assertion metadata. Migration must verify
those bytes against the preserved originals once, produce standalone ASP files
with a checked manifest, and preserve the original helper filtering contract
separately from complete-model parity. A refusal remains a refusal, not a pass
for language support. Keep MIT attribution when removing the copied C++ files.

The archival reference can identify the pre-migration Git revision and upstream
revision; future regression runs should verify the curated files directly. They
must not fetch third-party code or need a C++ compiler.

## Rust development tooling

Use library functions for fixture validation, bounded subprocess execution,
comparison contracts and report validation. The command layer should compose
those functions. Extend or extract the existing validation components before
creating a competing process runner. A small Cargo development-command entry
point is appropriate for repository maintenance; installed solver use remains
independent of Cargo.

Tooling is estate code and receives the same standard as the solver. This covers
development commands, test harnesses, benchmark drivers, fixture importers,
record validators and retained experiments. Keep domain concepts and error
outcomes explicit, compose small operations, name meaningful intermediate work,
and give each test one proposition. Apply the workspace formatting, pedantic
Clippy, documentation and relevant coverage gates. Review public tooling APIs as
deliberately as solver APIs. Neither a development-only location nor a mechanical
Python-to-Rust translation exempts code from these obligations.

Replace Python in this order:

1. Curated-fixture integrity and upstream regression execution.
2. Proof-record validation and coverage-floor checks used by CI.
3. Matched correctness/performance campaigns and their process-memory observers.
4. Still-relevant experiment preparation and device diagnostics.

The selected-upstream comparison now runs through the Rust `selected` library
and `zetesis-corpus compare`; its Python comparator, decoder and matching test
module have been removed. The new Rust `performance` library and `zetesis-perf`
command provide bounded three-case baseline and six-encoding queens CPU
measurements. They do not yet replace
the complete historical benchmark or process-memory tooling. The annotation-free
kr-domains collection and its typed contracts also have Rust integrity and
regression consumers. Remaining Python gates and C++ import provenance are still
tracked; this is not a completed foreign-source cleanup.

Every replacement preserves the existing contract: deadlines and output ceilings,
partial process evidence, failure classification, input/output alias protection,
complete model multiplicity, objective ties, source/binary hashes and coverage
floors, as applicable. Port the meaningful behavioral tests, run old and new tools
against the same fixed inputs, then switch callers and remove obsolete tooling.
A language migration alone does not establish better correctness or performance.

### Tool selection

The workspace already uses `proptest`, Criterion, `serde_json`, `sha2` and
`tempfile`. Reuse these where applicable. The immediate need is to replace the
Python orchestration and fixture machinery, not add another property-testing or
benchmarking framework.

Candidate tools were checked against their primary documentation on 2026-09-07:

| Tool | Suitable responsibility | Boundary |
| --- | --- | --- |
| [`assert_cmd`](https://docs.rs/assert_cmd/latest/assert_cmd/) | Command setup, exit status, stdin/stdout/stderr assertions for individual CLI tests. | Confirm subprocess limits and partial-output requirements before replacing the bounded campaign runner. |
| [`trycmd`](https://docs.rs/trycmd/latest/trycmd/) | Data-driven command fixtures and executable documentation examples. | Human-output snapshots do not replace typed comparisons of complete models, multiplicity and optimum ties. |
| [`cargo xtask` pattern](https://github.com/matklad/cargo-xtask) | A repository-owned Rust command for fixture, proof-record and campaign maintenance. | This is a development-command convention, not a solver API or a required installed-user workflow. |
| [`xshell`](https://docs.rs/xshell/latest/xshell/) | Small Rust development-command wrappers around external tools. | Avoid a second general subprocess implementation when existing bounded execution suffices. |
| [`cargo-nextest`](https://nexte.st/docs/configuration/reference/) | Test scheduling, timeouts, resource groups and machine-readable CI reporting. | Preserve doctests and feature configurations; serialize physical-device qualification and performance measurements. Keep retries disabled in correctness gates. |

Prefer one CLI fixture approach for each need rather than layering multiple
snapshot libraries over the same test. Evaluate candidate crates on a small
representative migration slice before adding dependencies throughout the estate.
The already implemented Rust process and report machinery is the first reuse
candidate. Full ASP comparison, evidence integrity and proof-record rules remain
zetesis domain logic, expressed as independently testable Rust library functions.

Separately, [`cargo-mutants`](https://mutants.rs/how-it-works.html) is worth a
targeted assessment of reduct acceptance, rollback and inclusive-limit tests.
It can expose behavior changes that existing assertions miss; a surviving mutant
can also be semantically equivalent and needs review. Mutation testing improves
the assessment of test strength but is not a replacement for Python scaffolding.

## Evidence retention

Keep evidence available while removing its bulk from the normal source tree.
Archive raw captures and historical scripts as immutable, checksummed artifacts,
with a manifest identifying source revision, toolchains, binaries, commands and
results. Verify the stored archive by reading it back before removing working-tree
copies. Retain concise reports and resolvable artifact references in Git.

Do not silently edit old reports, lose failed runs, break the current proof-record
gate, or make historical claims refer to newer binaries. Do not rewrite Git history
as part of this cleanup. Removing files from the latest tree reduces checkout size;
it does not remove those bytes from existing Git history.

While this migration is pending, new raw campaign captures belong under ignored
`target/` or external task scratch. Promote a concise report and the minimum
reviewable regression data needed for the claim, rather than another full copy
of a harness, baseline source tree, or subprocess transcript collection.

## Completion criteria

The ordinary checkout contains no Python, C++ or Objective-C development source;
the portable and proof checks no longer require Python. All curated fixtures run
from this repository alone. Raw historical evidence is accessible through verified
artifact indexes. Source, proof, regression and experiment responsibilities have
distinct documented homes. Existing semantic, resource, coverage and CI checks
pass after each migration slice, with no reduction in their obligations.
