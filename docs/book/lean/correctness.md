# Proving the solver correct

The target is a chain from an admitted program to the answers and conclusions
zetesis publishes. The reduct supplies the central membership argument. Grounding,
candidate coverage, representation and execution must establish the premises of
that argument for the actual implementation.

This chapter is a proof plan, not a claim of end-to-end verification. The
[correspondence inventory](correspondence.md) records individual boundaries.
The [theorem map](theorems.md) identifies results already checked in Lean.

## State the conclusions separately

Let `P` be a fixed program under the declared language semantics. Let `M` be a
full interpretation after decoding internal atoms and reconstructing any
definitions omitted from search. The principal obligations are:

1. **Answer soundness.** Every published answer `M` is an answer set of `P`.
   This holds even when the run later stops or fails.
2. **Complete enumeration.** When unrestricted search and answer retention
   both complete, the retained family is exactly `WorldView(P)`.
3. **Inconsistency.** An inconsistency result requires complete original search
   with no answer. An unfinished search does not establish it.
4. **Optimization.** A proved optimum is an answer set whose declared objective
   vector is no worse than that of any answer of `P`, or of an explicitly stated
   semantic query restriction. Complete optimal-tie
   enumeration additionally requires all tied answers to be covered.
5. **Observation and failure.** `#show`, `#project`, output errors, cancellation
   and resource limits preserve the distinctions above. A displayed projection
   is not the full interpretation; a projected representative family is not
   the unrestricted world view.

These are different theorems. Finding one valid answer does not prove complete
search, and proving a mathematical decision procedure does not prove that every
bounded implementation run terminates successfully. Enumeration order may vary
with scheduling without changing the family of answers.

## The membership argument

For a finite formula theory `T`, the defining obligation is:

```text
AnswerSet(T, M)
    iff satisfies(M, T)
        and no proper subset J of M satisfies the reduct T frozen at M.
```

The proof follows that definition:

1. Interpret every stored atom and formula node unambiguously.
2. Prove that the original evaluation computes `satisfies(M, T)`.
3. Prove that freezing at `M` and evaluating at `J` computes
   `satisfies(J, reduct(T, M))`; `M` remains fixed throughout the search.
4. Prove that the tested subsets cover every proper subset, and that early
   termination or interrupted work cannot fabricate exhaustive failure.
5. Compose those results with the independent answer-set definition.

`ReductEvaluation`, `IndexedEvaluation`, `SubsetCounter`, `PackedSubsets` and
`CounterSearch` establish this chain for authored executable Lean algorithms.
`PackedCounterSearch` composes packed updates and streaming control: every
completed verdict is exact. `TheoryAdmission` derives evaluator index bounds
from ordered finite checks. Refinement of the actual Rust operations remains
a separate obligation. The separately built [implementation-refinement package](evaluation.md)
connects the actual generated reference checker's completed public verdicts to
the same ASP theory under fixed observation tokens and explicit library contracts.

`MembershipSearch.completed_answer_set` composes actual original evaluation,
a successful original root check, completed atom selection and completed
proper-subset search. With represented candidate storage, ordered nodes, bounded
roots, matching atom counts, an empty selection vector and zero-filled packed
subset storage, search
returns no countermodel exactly when the candidate is an answer set of the
asserted theory. Work passes from each phase to the next in source order. The
proof derives mask meaning and selected-atom coverage, then constructs the actual
search execution using query correctness and the packed counter's rank argument.
Typed stops retain their actual state and work and do not establish exhaustion.

`PublicMembership.completed_answer_set` now derives those phase calls from the
actual completed public wrapper. Given exact candidate storage, ordered nodes,
bounded roots, a common consistent immutable heap and empty-sequence preservation
by the two successful reservations used without clearing, the returned `Stable`
verdict is equivalent to the answer-set definition. The negative results carry
the actual asserted false root or program-owned proper-subset reduct model.
The proof assumes neither allocation success nor correct inner-call results.
It does not yet relate the generated pure provider and fixed observations to
changing runtime histories. Source grounding, candidate enumeration and optimized
checking routes also remain separate. `AdmissionValidation` and
`AdmittedData` prove the actual admission step that `Theory::new` runs before it
allocates, including its node and root validators, so admitted data supplies the
ordering and root-bound premises. `TheoryConstruction` derives these from a
successful generated constructor return under the trusted allocation-value
contract. Its explicit provider describes one invocation; multiple-allocation
histories remain outside this proof. `InterpretationConstruction` connects the
actual interpretation constructor's reservation, initialization, iterable
conversion and insertion phases and proves retention of the supplied theory.
`InsertionLoop` proves actual finite insertion, including exact bounded-prefix
writes and first-invalid refusal. `InterpretationStorage` derives valid input
bounds, exact packed contents and zero padding from successful construction under
successful-reservation sequence preservation. `VectorInput` supplies the finite
contract for owned vectors; unrestricted iterator termination is not claimed.

`PublicFrozenQuery.constructed_satisfaction` composes actual successful public
stored-reduct construction and query calls. It derives the mask invariant and
proves that the query decides reduct satisfaction, given represented inputs,
ordered nodes and bounded roots. Construction and query receive separate
reservation providers, limits and fixed control tokens; no allocation-success
law is assumed. This closes that producer/query obligation. Its per-invocation
contracts and the completed public-membership result remain distinct from
correspondence to changing allocator and control histories.

`SubjectMembership.completed_subject` now composes successful theory and
owned-vector interpretation construction with the actual `check_interpretation`
API. It derives structural and storage premises and retains the exact checked
candidate. `completed_stable` proves that successful conversion of that decision
returns the same candidate as an answer set. These results retain the explicit
library contracts and fixed-observation boundary of the generated checker.

For normal rules, the positive reduct has a unique least consequence set.
`FiniteClosure`, `PackedClosure` and `PackedAcceptance` prove constructive
closure and acceptance results. Their link to general reduct semantics is
mathematical, through the [normal-rule correspondence](normal-rules.md).
Positive disjunctive theories need minimal-model checking; they do not generally
have a least model. Specialized tight or stratified routes require a proved
applicability test and equivalence to the same answer-set property.

## Connect source programs to the checked theory

The source semantics must be specified independently of the compiler being
verified. Defining the meaning of a program to be whatever its current compiler
produces would conceal compiler errors.

For eager grounding, prove a source-to-theory correspondence of this form:

```text
AnswerSet(P, M)
    iff there exists I such that
        AnswerSet(compiledTheory(P), I) and decodeAndReconstruct(I) = M.
```

This is schematic notation for the target theorem. A translation with auxiliary
atoms needs the stated projection/reconstruction law; a bijection must not be
assumed without proof. The source contract includes the documented language
extensions and refusals, rather than undocumented behavior of another solver.

The concrete obligations include complete finite substitutions, local variable
scope, typed term identity, checked arithmetic, strong-negation consistency,
choices, aggregates and optimization contributions. Domain analysis and
semantics-preserving rewrites must retain every relevant substitution and every
answer. Classical formula equivalence alone does not justify replacing a
formula inside a Ferraris theory.

For lazy or hybrid grounding, use a simulation invariant instead of requiring
the complete ground theory to be materialized. Every emitted instance must be
valid, retained obligations must cover what remains, and an acceptance or
completion claim must discharge the relevant obligations against the original
program. An instance not yet generated is not thereby false. The implementation
must prove the scheduling and saturation conditions used by that invariant.

The first source-level theorem can begin at an admitted themelios program
representation. Text parsing and raising then remain an explicit upstream
boundary. This does not require implementing themelios functionality in zetesis.

## Refine the actual execution

| Boundary | Concrete proof obligation |
| --- | --- |
| Canonical storage | Typed term and atom identities, lookup completeness, stable IDs, owner identity, exact packed lengths and padding |
| CPU primitives | Actual reads, writes, joins, filters, reductions and fixed-point scans preserve their mathematical denotations |
| Candidate generation | Initial coverage and every split, propagation or pruning step preserve all possible original answers |
| Specialized checking | Recognizers establish the advertised program class; a fast acceptance is justified, and unresolved cases receive exact completion before a verdict or remain unfinished |
| Resource handling | Successful operations preserve invariants; stopped operations cannot establish rejection, exhaustion or optimality |
| Parallel execution | Tasks cover the pending search without lost work; results remain attached to the same program, candidate, reduct and request |
| GPU execution | Packing, kernel operations, synchronization and readback implement the same primitive contracts; residual work receives exact completion before a verdict or remains unfinished |
| Results | Reconstruction, scoring, projection, retention and delivery preserve the verified answer and its completion evidence |

An invariant for a join or bit operation is proved once at the shared primitive
boundary. Each consumer still must establish its input premises. Sharing a
library function or using the same operation name does not itself establish
that two execution paths implement the same semantics.

Parallelism adds a scheduling proof around these operations. Soundness must not
depend on a favorable schedule. Eventual completion additionally needs explicit
finite-work, sufficient-resource and scheduling-progress assumptions; operating
system or device progress is not obtained from a set-theoretic coverage law.

## A practical sequence of verified milestones

1. **Public scalar reference checker.** Establish admitted input and initial
   storage invariants, preserve program ownership, and connect the public
   wrapper to the proved reference-checker phases. Theory construction and
   stored-reduct construction/query are covered under per-invocation library
   contracts. Interpretation construction now derives exact packed contents
   and padding for finite observed inputs, with the input contract proved for
   owned vectors. Completed public membership verdicts now compose the actual
   wrapper's setup and semantic calls under explicit library contracts. Relate
   completed and refused calls to permitted runtime allocation and cancellation
   histories. The subject-bound `check_interpretation` API and its accepted-result
   conversion now compose successful constructors with the membership theorem.
   This milestone remains open. Unrestricted iterator termination is not an admission
   guarantee.
   This milestone concerns one candidate of a finite ground formula theory.
2. **Optimized CPU checking and enumeration.** Connect normal closure and each
   optimized membership route to the same answer-set definition. Prove candidate
   generation, plan selection, exact classification and completion accounting.
   A complete retained family must equal the original finite theory's world
   view. Each specialization needs its recognizer and preservation argument.
3. **Source compilation.** Start from an admitted program in a declared finite
   normal-language fragment. Prove concrete grounding and reconstruction, then extend the theorem
   construct by construct to the admitted language. Include arithmetic errors
   and incomplete preparation, not only successful ground output.
4. **Lazy execution and concurrency.** Refine the demand/saturation and task
   accounting invariants against real transitions. Preserve the same source
   semantics, including pending work and interrupted publication.
5. **Device and remaining result paths.** Prove the WGSL/backend correspondence
   and compose reconstruction, objective and observation refinements with the
   same membership and coverage results. Qualify each covered route explicitly.

This order yields useful named results before the entire solver is verified.
Proofs of a shared primitive can proceed concurrently, but completion claims
follow the dependencies. Each milestone must replace an assumed correspondence
with a checked implementation argument; another conditional composition theorem
alone does not close that obligation.

## State the trust boundary

Every implementation result must name the checked source revision, supported
routes, admitted inputs and modeled effects. It must also identify its remaining
trust: Lean's kernel and permitted logical axioms, extraction/translation tools
and library models, upstream parsing where assumed, and the compiler/runtime,
driver and hardware semantics beneath the chosen machine model.

The verification target assumes Rust's allocator, `Arc`, the operating system
and hardware implement their contracts correctly. Zetesis must still be proved
to use those contracts correctly. Allocation may fail, and cloning an owner
preserves its identity without making distinct, equal-valued owners identical.
Models of these contracts must retain those distinctions; verifying the platform
implementations themselves is outside this project.

Clingo comparisons, property tests, coverage and physical-device qualification
remain useful independent evidence. They do not replace a refinement proof.
Conversely, a proof about a narrower model does not qualify unmodeled allocation,
concurrency or device behavior. The release claim must describe the verified
boundary exactly.
