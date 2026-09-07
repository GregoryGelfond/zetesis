# Public API review — 7 September 2026

## Decision

Complete a bounded API hardening checkpoint before the next language-feature or
optimization tranche. The reusable reduct engines, source analysis and model
views are useful foundations. The ordinary execution boundary does not yet carry
all the distinctions that Rust consumers need without reconstructing them from
CLI behavior. The review also found a reproducible output-resource contract defect.

This is a read-only review of `55c37de99de1fe8f58d11d2a918417d593aaae44`, conducted
across semantic values/membership, orchestration/outcomes, and frontend/views,
with a separate backend/domain-analysis review. Findings were integrated against
public signatures and implementation paths. Only the human byte-limit example
below was executed for this review; the other witnesses are source traces.
[Review fragments and evidence](../verification/public-api-review-20260907/README.md)
preserve that distinction. No new native stable-model soundness defect was
established, and this is not an exhaustive audit of every public declaration.

## Priority findings

The ordering below is a proposed repair dependency, not a severity score for
operational deployment. “Contract defect” means observed behavior or inspected
code contradicts a stated public claim. “Representation gap” means the current
interface requires discipline that a stronger composition boundary could retain.

| Order | Finding | Evidence and classification | Required response |
| --- | --- | --- | --- |
| 1 | Plain human answers bypass the complete-record byte ceiling. | **Executed contract defect.** With `a.` and `--max-observation-bytes 0`, the release publishes an Answer and exhausted SAT. Adding an evaluated `#show` term refuses before publication. `display.rs` takes an unbounded early branch when observations are empty. | Give every human record one bounded preparation/publication boundary, retaining the fast empty-observation semantic path. |
| 2 | Observation output accounting mixes logical term payload and retained input representation. | **Source-traced accounting contract mismatch.** Structured bound values use capacity-sensitive `payload_bytes`, including cached spelling, where observation limits describe per-node and UTF-8 payload charges. | Define input-storage, logical retained-term and output-byte accounting separately; preserve conservative allocation bounds while repairing the documented measure. |
| 3 | A successful public `Report` loses verified-model counts that internal progress, JSON and failed reports retain. | **Representation gap.** Verification can exceed publication when a batch is checked before a model cap or sink failure. Neither published count nor candidates checked can reconstruct it. | Expose one finalized semantic outcome on success and failure, separate from publication acknowledgements. |
| 4 | Exhaustion wording can imply UNSAT from zero published models after an output failure. | **Documentation defect.** Exhausted search, one verified model and zero published records is already covered by a regression. | Define exhaustion as coverage of its stated search region. Derive UNSAT only from complete relevant semantic evidence; publication failure cannot establish it. |
| 5 | Raw interpretations and reported verdicts can be mistaken for checked facts. | **Taxonomy/construction gap.** `Model::new` only canonicalizes atoms; public `Check::Stable` is constructible without checking, despite proof-oriented wording. | Introduce accurate raw-interpretation vocabulary and a separate private-field checked-result door tied to its subject. Keep raw verdict data available with an explicit producer trust contract. |
| 6 | Dense result words lose program identity when transported. | **Association gap.** Same-shaped words can be decoded against a different graph. Existing callers are required to retain the pairing; no wrong pairing in native solving was found. | Carry graph/theory and candidate association in a checked result or higher-level batch result, without copying whole programs. |
| 7 | The ordinary driver cannot start from already admitted or ground input. | **Composition gap.** Source and bundle entry points still combine preparation, execution, objective retention and writing through clap `Options`. | Add coherent prepared-input doors and extract a writer-free session over the same engines. Keep source/CLI adapters. |
| 8 | Some terminal evidence survives only in the caller's history. | **Composition gap.** `Candidates` yields a stop and then fused `None`, without a retained terminal-reason query. Multiple secondary reporting failures share one slot. Cancellation classification varies by phase. | Retain a bounded terminal/delivery record with explicit stop scope and separate diagnostic/summary acknowledgements. Preserve the existing iterator contract during migration. |
| 9 | Output and hardware vocabulary exposes presentation conventions as data. | **API usability/representation gap.** `OutputSelection` selects atoms only; view statistics count observation work only. GPU metadata uses absent strings and backend/category labels. | Refine atom-selection/observation-accounting names and typed optional adapter metadata through additive accessors and adapters. |

The detailed traces and proposed regression obligations are in the
[orchestration](../verification/public-api-review-20260907/orchestration.md),
[semantic](../verification/public-api-review-20260907/semantic.md),
[view](../verification/public-api-review-20260907/views.md) and
[backend](../verification/public-api-review-20260907/backend.md) reviews.

A public enum containing `Stable` is not itself a checking receipt. Conversely,
a deliberately injected checker has a documented soundness precondition;
violating that precondition is not evidence of a native oracle defect. A Rust
private-field result can establish an implementation construction invariant; it
does not constitute a Lean proof of Rust, source lowering or a GPU shader.

## Vocabulary and ownership to preserve

| Concept | Meaning at the library boundary |
| --- | --- |
| themelios program | Source-level logical structure and provenance before admission or grounding. |
| Admitted input | A coherent immutable prepared owner with its source identity, metadata and semantic representation; no freely paired theory/atom table. |
| Ground representation | An explicitly scoped materialization. The static closure graph and general finite formula theory are different representations with different admitted profiles. |
| Interpretation | An exact truth assignment in a stated universe, or a canonical symbolic atom set; construction alone establishes no satisfaction or stability. |
| Seed | The instance-bound gate-carrier projection used by closure checking. It is not necessarily the whole resulting answer set. |
| Checked membership | A result produced by the relevant oracle for that same program/theory and candidate, retaining acceptance, rejection or incompleteness. |
| Coverage | The examined search region plus terminal evidence. Restricted exhaustion needs an additional justification before implying a global result. |
| Semantic outcome | Finalized verification, scoring, retention, coverage, interruption and optimum evidence, independent of rendering. |
| Publication | Successful delivery of complete records to a sink. It can fail after semantic work completes and does not imply durable storage. |

Names here identify concepts, not proposed long identifiers or a mandatory crate
split. Prefer existing estate vocabulary where it expresses the same invariant.
The comparison with morphe's source and already-parsed entry points supports
reusing established structure. themelios constructors that enforce their named
invariants and its provenance-preserving Program provide a stronger model than
merely adopting similar spellings. Sibling projects were read only.

Domain analysis already borrows the exact themelios Program and distinguishes
Unknown from a finite empty domain. Its global fallback clears finite bounds.
That safe loss of precision should remain. GPU APIs already separate original
model rejection, proper-subset refutation, unresolved propagation and device
failure; explicit adapter requirements are not silently replaced by CPU work.
Preserve these semantic distinctions through the new execution boundary.

## Proposed checkpoint and exit criteria

1. **Repair public resource and coverage contracts.** Prepare every human record
   within its declared limit before publication. Reconcile observation accounting
   with its precise measure. Preserve output bytes at sufficient limits and exact
   refusal/accounting at insufficient limits. Clarify exhausted-region semantics.
   Add one-proposition regressions for each boundary, including selected fault
   injections, without weakening coverage floors or hiding allocator costs.
2. **Expose finalized semantic evidence and bound checked subjects.** Preserve
   verified, scored, retained and delivered counts independently. Add accurate raw
   interpretation accessors and an instance-bound checked-result API. Existing
   public fields/types need an additive compatibility path; do not expose private
   `Progress` with its provisional completion state, or wrap user-provided verdicts
   into alleged checked evidence.
3. **Compose a prepared-input, writer-free session.** Extract existing execution
   loops and budget ownership into reusable Rust operations. Keep CLI options,
   source loading, human/JSON formatting and sink acknowledgements in adapters.
   Demonstrate a Rust client that solves an admitted input without source replay,
   a writer or clap configuration. Compare it with the existing wrappers using
   full interpretations, cost vectors, optimal ties and terminal evidence.
4. **Review compatibility and costs before freezing names.** Keep old doors while
   testing adapters. Audit per-call/per-candidate/per-session limits, retained
   memory, pending errors and exact identity. Run existing parity/property tests,
   strict Rust checks and independent coverage gates. Measure any changed output
   preparation cost against matched inputs rather than assuming extraction is free.

The first session extraction should not add resumability, multi-shot updates,
theory extensions or ASPIF together. Each needs additional identity, lifetime,
coverage and cumulative-budget rules. The architectural doors should accommodate
those later uses without pretending they are already implemented. No CDNL search
or replacement of the reduct architecture is proposed.

Lean work should state the abstract transitions for candidate admission,
checked membership, pending/committed accounting, restriction and finalization.
Existing `CertifiedExecution` and `BatchAccounting` laws supply scoped ingredients.
A new result's introduction rule must correspond to the actual producer and same
subject; a terminal coverage claim must include all pending decisions and stops.
Keep publication acknowledgements outside mathematical membership. Record the
remaining executable-refinement premises rather than presenting an API wrapper
as a machine-checked implementation proof.

## Assessment

The standards application has improved the project: it fixed timing-integrity
logic, made weak tests falsifiable, clarified contracts and prompted this review,
which found an independently reproduced output-bound defect. Naming is useful
when it exposes an unestablished claim or an absent concept; shorter identifiers
alone are not evidence of reliability.

The current release's qualification remains valid for its stated tests: all four
hosted jobs, the two line-coverage floors, and the 94-case CPU corpus campaign pass.
That does not close these newly identified API obligations or establish
mission-critical fitness. Complete the bounded hardening checkpoint before the
next feature/optimization tranche and before promising a stable public API.
