# A mathematical ASP library and a verified solver

Status: verification direction and candidate milestones, not an expanded release
commitment. Release work is restrained to the fundamentals enabling formal
verification of zetesis, designed to fit a broader stable-model-semantics library.

The long-term goal has two deliverables: a reusable, readable Lean library of ASP
semantics and algorithms, and an executable solver connected to those semantics
by checked refinement or independently checked result certificates. Neither
requires changing zetesis's present search algorithm. Original-program
satisfaction and frozen-reduct minimality remain the acceptance criterion.
The adjacent [ASP theory library](asp-theory-library.md) aims more broadly to
formalize published results with exact attribution, assumptions and reusable
proofs/counterexamples; the solver is one client. General theory stays independent
of packed IR and backends. A separate package is a future option, with no
extraction now and no new version-1.0 prerequisite.

The [current audit](../../proofs/verification/dependency-tranche-20260907/README.md)
checks 702 theorems in 53 modules. This is a substantial mathematical foundation,
not a verified Rust solver. Several laws assume complete carriers, faithful
encodings or sound classifiers; proving the implementation establishes these
premises is the next kind of work. The theorem count does not measure that gap.

## Prior work and the novelty question

This bounded primary-source review was conducted on 2026-09-07, searching for
verified ASP solvers, proof certificates, and Lean/Isabelle/Coq formalizations.
It found directly relevant ASP work as well as adjacent verified-solver work:

| Work | Established contribution and boundary |
|---|---|
| Lierler, [Abstract Answer Set Solvers with Backjumping and Learning](https://digitalcommons.unomaha.edu/compscifacpub/4/) (2011) | Transition-system accounts of several ASP algorithms support correctness reasoning. An abstract algorithm account is distinct from a kernel-checked executable refinement. |
| Alviano et al., [ASP-DRUPE](https://arxiv.org/abs/1907.10389) (2019) | A sound and complete inconsistency-proof format for propositional disjunctive programs, including weight/choice rules; the paper gives a checker and reports a wasp implementation for normal programs. The authors retain a [checker implementation](https://github.com/alviano/python/tree/master/asp-proof). This establishes prior ASP certification, not a verified source-grounder-executable chain. |
| VUB, [CertifASP](https://researchportal.vub.be/en/projects/certifasp-certified-answer-set-programming/) (2024–2028 project) | An active project explicitly targets proof systems, checkers and proof logging across ASP solving approaches. Its project description is an aim, not evidence that a fully verified ASP implementation has already been delivered. |
| Kaminski and Schaub, [On the Foundations of Grounding in ASP](https://arxiv.org/abs/2108.04769) (2021/2022) | Fixed-point foundations and algorithms for grounding, including recursive aggregates. This supplies grounding theory to study; it does not verify zetesis's source compiler. |
| Cabalar, Fandinno and Lierler, [Modular ASP as a Formal Specification Language](https://arxiv.org/abs/2008.02015) (2020) | Verifies that ASP encodings express their intended problems. Correctness of an encoding is separate from correctness of the solver executing it. |
| Blanchette et al., [A Verified SAT Solver Framework](https://matryoshka-project.github.io/pubs/sat_article.pdf) (2018) | Isabelle refinements connect abstract calculi to functional and imperative SAT algorithms with total-correctness guarantees. This is a useful methodological precedent, not ASP verification or a reason to replace our search algorithm. |

We should not claim “first formally verified ASP solver” or “first Lean ASP
library.” This search does not establish either priority claim; missing search
results are not evidence of absence. Any later novelty statement must specify
language fragment, semantic scope, proof system, executable correspondence and
trusted components, then undergo a broader literature/artifact review.

## Claims that must remain distinct

**Mathematical laws** describe programs and interpretations. A **verified
algorithm** additionally proves its operations sound and complete, with explicit
termination premises. A **verified executable** connects those operations to
concrete representations, mutation, arithmetic and execution semantics. Merely
copying a proved algorithm into Rust does not establish that connection.

A **certifying solver** supplies evidence for particular results. A verified
checker can validate that evidence without trusting the search heuristic. It
does not thereby prove that search always terminates, finds every answer set,
or stays within a promised memory bound. Likewise, current private-field Rust
receipts retain native check identity; they are not Lean proofs.

## Staged roadmap

1. **Consolidate the mathematical library.** Keep stable logical names, explicit
   premises and readable proofs under the existing [style](../../proofs/STYLE.md).
   Organize semantic laws, executable definitions and representation theorems so
   each can be imported independently. Reuse `Ferraris`, `Semantics`,
   `LiftedBridge`, `Search` and `Outcomes`; maintain a correspondence inventory
   naming the implementation producer for every assumed coverage/identity fact.

2. **Verify one executable ground-program boundary.** Start with finite normalized
   rules and a finite atom catalog. Implement a total Lean checker and prove that
   successful acceptance is exactly reduct stability, using least closure in
   the eligible normal profile. Define bounded decoding and interpretation
   identity explicitly. The first claim concerns the received ground program,
   not arbitrary clingo source. Extend to finite Ferraris membership with checked
   original truth and exact proper-subset refutation. Exhaustive finite checking
   is a reference, not an efficiency promise.

3. **Connect execution and completed search.** Choose and document either an
   extracted implementation or a proof of concrete Rust refinement; a handwritten
   port is insufficient. Alternatively, prove a certificate checker and keep
   existing search as the producer. General reduct acceptance needs evidence
   excluding every proper-subset countermodel, not merely a satisfying assignment.
   Check pruning and candidate-region coverage as well as individual answers.
   Existing `CandidateCursor`, `BatchAccounting` and `CertifiedExecution` laws
   provide contracts, not this implementation connection.

4. **Prove source and grounding correspondence incrementally.** First connect an
   admitted finite relational fragment to its conceptual grounding. Then cover
   signed identity, values, assignments, choices, aggregates and conditionals.
   Parsing, includes, constants, undefined arithmetic and source parameters need
   explicit interpretations. For lazy execution, a sound generated prefix is
   insufficient: final-snapshot rule/witness coverage and candidate-domain
   coverage must both hold. Prepared-input receipts and matching artifact hashes
   do not prove these obligations.

5. **Extend verified conclusions and measured execution.** UNSAT requires complete
   coverage with no answer set, not empty output. An optimum needs verified
   membership, exact objective ordering and exclusion of every better answer;
   enumerating all optimal ties needs additional complete tie coverage. Prove
   these alongside distinct stopped, failed and publication outcomes. Resource
   exhaustion may preserve verified answers while withholding completion.
   Mathematical termination under finite inputs does not promise a wall-clock
   deadline or recovery from every allocator/OS failure.

## Parallel and GPU trust

One possible route treats CPU/GPU workers as untrusted certificate producers.
The trusted boundary checks complete atom/program/query identity and every
conclusion used for acceptance or pruning. Checking returned models alone cannot
detect silently omitted candidates or unsound device pruning. The design still
needs coverage, source-grounding and transport/refinement proofs; replaying only
an already decided CPU closure would not establish useful lazy GPU execution.

Current projection and event laws do not verify WGSL, atomics, drivers or hardware.
A certificate route can reduce what must be trusted for semantic correctness,
while device failure remains an explicit outcome. Physical qualification and
measurements must separately establish actual device work, transfer/checking
costs, bounded memory and useful performance.

For every milestone, publish its exact accepted syntax, proved theorem, executable
identity and remaining trusted components: kernel/axioms, decoder, compiler,
runtime, FFI and hardware as applicable. Compiled Lean code also requires an
honest compiler/runtime boundary. A proposed first executable-verification milestone
is a small membership checker with a checked semantic theorem and explicit input boundary;
the full source-to-result solver claim comes only after the remaining links exist.
