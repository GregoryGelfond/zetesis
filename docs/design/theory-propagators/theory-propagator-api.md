# Proposed semantic extension interface

Companion to [the architecture investigation](theory-propagators.md).
2026-09-06. **Non-compiling design sketch**, not implemented Rust API or a
committed crate roster. Names below describe obligations to reconcile with
themelios-solve's final shared contract. No registration or theory support is
advertised by the current solver.

The distinction to preserve is between a theory language's semantics, an
immutable prepared instance, an optional propagation accelerator, and exact
witness completion. A plugin callback cannot declare itself to have proved
Boolean stability.

```rust
// Conceptual shared extension vocabulary; fields are private in the final API.
struct TheorySchemaId { /* registered language and semantic revision */ }
struct ExtensionVersion { /* owner token + source/context/schema identities */ }
struct TheoryAtomId { /* version + catalog index */ }
struct TheoryVariableId { /* version + catalog index; typed domain */ }
struct ConstraintId { /* version + catalog index + origin */ }
struct CandidateId { /* version + unique proposal identity */ }
struct BranchId { /* candidate + numerical branch generation */ }

enum AtomPolicy {
    ExternalStrict,
    FoundedNonStrict,
}

// A role in the engine's immutable lowering, not an author-selected search mode.
enum SemanticProfile {
    BooleanReductWithWitness(ValidatedProfileId),
    FiniteCompiled(ValidatedEncodingId),
    // A future extended-reduct family must add its own typed input/checker.
}

struct PreparedExtension {
    version: ExtensionVersion,
    catalog: TheoryCatalog,       // source keys, typed terms, Origin, atom policy
    constraints: ConstraintStore, // immutable, exact admitted payloads
    domains: DomainDefinitions,   // definitions preserve conditional activation
    semantics: SemanticProfile,
    objectives: TheoryObjectives,
    observations: TheoryObservations,
}
```

The engine creates `PreparedExtension` only after source/theory admission. A
profile ID is not a user assertion that an arbitrary compiler is correct:
supported profile constructors are tied to specific reviewed lowering/checking
implementations. This still leaves those implementations in the trusted base.
`AtomPolicy` names semantic roles; merely providing an enum variant does not
admit every syntactic occurrence. In particular, the cited 2023 paper excludes
founded theory atoms from rule bodies and uses a single atom/falsum head. A
profile using that correspondence must enforce its restrictions. Arbitrary
Ferraris contexts and founded-body cycles need separate semantics/refinement;
the report's `f :- f.` example is an internal boundary counterexample, not an
asserted admitted clingcon program. Current runtime theory refusals remain.
Persistent hashes describe semantic content; in-process tokens establish that
indices belong to the same object. Neither is an authorization to reinterpret
someone else's IDs.

Direct source admission and themelios's typed `Door::Program` eventually produce
the same prepared instance. Theory definitions resolve operators without a new
parser; language-specific normalization works on typed theory terms. Refusal
retains the input `Origin`. Lazy preparation additionally returns a coverage
obligation, never a falsely complete `GroundProgram`.

```rust
// Generic author-facing registration can wrap this into an object-safe factory.
trait TheoryExtension: Send + Sync + 'static {
    type Worker: TheoryWorker;

    fn capabilities(&self) -> TheoryCapabilities;
    fn prepare(
        &self,
        input: TheoryInput<'_>,
        limits: &mut PreparationAllowance,
    ) -> Result<PreparedExtension, TheoryFault>;

    fn worker(
        &self,
        prepared: Arc<PreparedExtension>,
        allowance: &mut WorkerAllowance,
    ) -> Result<Self::Worker, TheoryFault>;
}

// Private adapter erases Worker, rather than requiring callers to name State.
trait ErasedFactory: Send + Sync {
    fn worker(
        &self,
        prepared: Arc<PreparedExtension>,
        allowance: &mut WorkerAllowance,
    ) -> Result<Box<dyn TheoryWorker>, TheoryFault>;
}

trait TheoryWorker: Send {
    fn begin(
        &mut self,
        request: TheoryRequest<'_>,
        allowance: &mut WorkAllowance,
    ) -> Result<(), TheoryFault>;

    fn propagate(
        &mut self,
        view: CandidateDomainView<'_>,
        output: &mut BoundedEffects,
        allowance: &mut WorkAllowance,
    ) -> Result<PropagationProgress, TheoryFault>;

    fn checkpoint(&self) -> RollbackMark;
    fn restore(&mut self, mark: RollbackMark) -> Result<(), TheoryFault>;

    fn advance_witnesses(
        &mut self,
        output: &mut BoundedWitnesses,
        allowance: &mut WorkAllowance,
    ) -> Result<WitnessProgress, TheoryFault>;
}

enum WitnessProgress {
    More,                    // may have produced zero witnesses this quantum
    Exhausted(TheoryEnd),     // complete coverage, not just an empty output batch
    Suspended(Interruption),
}
```

`begin` resets version/candidate state. Its request explicitly names existence,
all assignments, or numerical optimization. A worker may decline unsupported
requests before work. `advance_witnesses` returns owned typed valuations tagged
with version and candidate; buffers are size/byte bounded. Only the coordinator
publishes answers. An optional fork operation can be added once snapshot/replay
semantics and aggregate scratch limits are established; arbitrary `Clone` of a
worker is not assumed to be correct.

These operations implement themelios's intended init/propagate/undo/check roles
at an adapter boundary. The final public trait can keep the draft's method names
while its contexts expose the same distinctions. Incremental numerical search
is not disguised as a Boolean `check() -> bool`; a total Boolean assignment may
still leave substantial numerical work.

```rust
enum Effect {
    Implication {
        premises: LiteralSet,
        consequence: SemanticLiteral,
        explanation: Explanation,
    },
    NarrowDomain {
        premises: LiteralSet,
        variable: TheoryVariableId,
        remaining: DomainRestriction,
        explanation: Explanation,
    },
    Conflict { explanation: Explanation },
    BranchHint(BranchSuggestion),
}

enum ExplanationScope {
    CandidateVersion(ExtensionVersion),
    CandidateRegion(RegionId), // assumptions/objective identity are inside RegionId
    FrozenReduct(FrozenQueryId),
}

trait WitnessValidator: Send + Sync {
    fn validate(
        &self,
        prepared: &PreparedExtension,
        candidate: CandidateView<'_>,
        witness: &TheoryAssignments,
        allowance: &mut WorkAllowance,
    ) -> Result<ValidatedWitness, WitnessError>;
}

trait ExplanationValidator: Send + Sync {
    fn validate(
        &self,
        prepared: &PreparedExtension,
        claim: &Effect,
        context: ExplanationContext<'_>,
        allowance: &mut WorkAllowance,
    ) -> Result<ValidatedEffect, ExplanationError>;
}
```

The prototype's ordinary theory worker can only submit candidate effects.
`FrozenReduct` is reserved for a separate specifically admitted validator and
operator family. Typed scopes prevent accidental mixing but do not prove a
claim. `ValidatedWitness` and `ValidatedEffect` have no unchecked public
constructors. If arbitrary custom proof checkers are accepted, they are part of
the trusted base; naming their output validated does not remove that fact.

Effects are admitted after each bounded compute step. If effect admission fails,
the coordinator retains the unresolved branch or restores its checkpoint. It
must not count a locally removed domain as globally committed. A branch hint can
change traversal order only; it cannot silently discard a branch.

```rust
struct JointAnswer {
    version: ExtensionVersion,
    regular: themelios_program::AnswerSet,
    assignments: TheoryAssignments,
    cost: ExactObjectiveVector,
    identity: JointModelId,
}

struct HybridProgress {
    boolean: BooleanCoverage,
    theories: TheoryCoverage,
    published: PublicationProgress,
}

struct ExtensionFailure {
    cause: TheoryFault,
    partial: HybridProgress,
    // Retained cursors/answers remain owned by the run or an explicit resumable handle.
}
```

The final shared envelope may have a different name/home. It must preserve
joint identity and the split between logical determination and search
conclusion, and it must distinguish witnesses found from exhaustive witness
coverage. Failure metadata must not borrow dropped worker state.

For device support, an optional capability returns a checked declarative
`TheoryPlan`, with a CPU reference meaning, a supported arithmetic profile,
bounded storage layout, and explicit residual protocol. It must not accept
arbitrary WGSL as a sound solver extension or promise to compile arbitrary Rust.
Opaque host extensions remain valid hybrid components when declared as such.

The API litmus is concrete: an integer CP worker returns domain restrictions and
assignments; a DL worker can return a cycle explanation or checked potential
vector; a rational LP worker needs exact certificates and a distinct outcome
for unboundedness or unattained optima. The host interface should not force each
to pretend that its state is a vector of Boolean decision levels, nor should
one scalar numerical type silently truncate the others.
