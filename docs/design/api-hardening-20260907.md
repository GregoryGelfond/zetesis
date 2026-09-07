# Semantic and execution API hardening — 7 September 2026

This implements the bounded repairs identified by the
[public API review](public-api-review-20260907.md). The review remains a record of
the previous release. This checkpoint changes resource admission and ordinary
execution composition; it adds no source-language feature or search algorithm.

## Boundaries

| Review obligation | Implemented boundary |
| --- | --- |
| Complete human record ceiling | Every Answer is prepared within its byte limit before external publication. Empty observation programs still consume no observation evaluation work. |
| Truthful observation accounting | Logical node/text charges are independent of input capacity. A separate construction budget admits conservative temporary and retained symbol storage. |
| Semantic results independent of delivery | `SemanticOutcome` preserves verified, scored and retained counts, coverage, interruption and incumbent evidence. `SolveReport` and `SolveFailure` expose it separately from complete record acknowledgements. |
| Coverage and UNSAT | Exhaustion describes the relevant search region. Ordinary session UNSAT additionally requires zero verified models; zero published records is insufficient. |
| Interpretations and checked membership | `Interpretation` names raw atom data. Native checked results bind reconstructed CPU closures to their program, and formula candidates to their theory; reported public verdicts remain data with a producer trust contract. |
| Transported result identity | CPU checks retain their program. Static checks reject foreign programs when decoding; checked scalar results provide subject-bound access without another membership check. |
| Reusable ordinary execution | `PreparedInput`, `SolveConfig` and `Session` accept coherent admitted or ground owners and yield typed models without source replay, command arguments or model writers. CLI adapters consume the same retained closure/formula loops. |
| Terminal and reporting evidence | CPU candidate iterators retain exhaustion or their stop; sessions retain their semantic outcome. Rich invocation failures preserve the primary cause and separate bounded diagnostic/summary failures. Publication cancellation has a typed phase-specific error. |
| Views and hardware vocabulary | Source-free atom selection, canonical themelios reexports, metadata-only compilation and detailed observation/JSON accounting compose independently. Adapter metadata exposes typed backend/category and optional text. |

Private constructors establish Rust construction invariants. They do not turn
untrusted callbacks into native checking receipts, or constitute a mathematical
proof of the implementation. Raw interoperability doors remain available and are
documented as such.

## Session ownership

Prepared inputs borrow a complete admitted owner or an already compiled ground
graph. Callers cannot independently pair a formula theory with a differently
indexed atom table. Strategy requests incompatible with the prepared profile
are refused. Each session owns fresh search budgets, worker resources, pending
batch results and incumbent storage; successive pulls retain that state.

Detached session models and established outcomes retain a cheap shared handle to
their original program or formula theory. Publication snapshots remain independent
of caller edits to legacy compatibility reports. Their interpretation includes atoms hidden by observations.
An objective score is a completed evaluation, not a claim that the optimum has
been proved. Objective search finishes or stops before retained ties are yielded;
its outcome can therefore precede delivery. An early caller stop does not invent
coverage. A batch can verify more models than the caller consumes. `candidate_progress()`
counts consumed closure results or proposed formula candidates, as its route-specific
contract states; it does not name either measure completed checking. Legacy
`Report.checked` remains available for compatibility.

The session API is initially exported by the existing `zetesis-cli` library.
Consumers need neither `Options` nor a writer, but the package still contains
command adapters and depends on clap. Separating that dependency closure into a
dedicated composition crate remains packaging work; this checkpoint does not
claim the entire application dependency graph is presentation-free. ASPIF,
multi-shot mutation, resumability and theory extensions remain separate designs.

## Costs and compatibility

Human record preparation performs two traversals of immutable spelling and
selection: count actual UTF-8 writes under the ceiling, then reserve and encode
the admitted record. It retains one record buffer, with allocator rounding outside
the logical ceiling. For observed terms, the existing bounded rendered view also
remains live. Cancellation is cooperative; counting polls during writes and the
adapter polls before external publication. Partial sink writes remain possible
and never acknowledge a complete model.

An exceeded human byte limit reports the first oversized prefix, a lower bound
on the full record size. It does not finish traversing an oversized answer merely
to compute an exact refusal number. Logical observation payload and construction
storage are distinct from encoded output bytes and process RSS. Construction
admission includes the canonical name validator's transient text copy. Detailed
JSON accounting preserves attempted work on failure; it does not claim constant
encoding cost or erase existing depth-dependent cursor costs.

Existing invocation functions and public `Report`, `RunFailure` and observation
`Limits` layouts remain available through adapters. New result doors preserve
additional evidence that legacy conversions explicitly discard. New error/resource
enum variants require downstream exhaustive matches to add cases. The plain-human
byte bypass is intentionally corrected, so formerly unbounded zero-byte requests
now fail. At sufficient limits, human and JSON record spelling stays compatible.

## Mathematical obligations

`Zetesis.Outcomes` adds three checked laws to the existing batch accounting
library. Complete sound coverage relates empty acceptance to regional
unsatisfiability; delivered members inherit validity from accepted members; an
empty delivery can coexist with a valid accepted model. The last law is a concrete
counterexample to deriving UNSAT from publication counts.

These bring the recorded library to 616 theorems in 42 modules. Their premises
explicitly distinguish coverage of a stated region from a global source-program
claim. They do not verify Rust, allocation, byte sinks, source lowering or shaders.

## Assessment and qualification

The standards application has produced concrete improvements: a reproduced byte
limit bypass is fixed, capacity-dependent logical accounting is corrected, and
review found a second temporary-storage omission before the code was frozen.
The new types preserve distinctions that previously depended on caller history
or CLI output. Shared stateful loops make budget ownership and publication order
easier to inspect without duplicating the solver algorithm.

This is progress toward the estate quality floor, not certification for military
or financial deployment. Language coverage, arithmetic performance, broader GPU
execution, deployment assurance and executable refinement remain open objectives.
The [qualification record](../verification/api-hardening-20260907/README.md)
distinguishes focused checks, integrated gates, ordinary performance and physical
device evidence. Historical Metal results retain their original binary identity.
