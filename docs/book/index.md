# The zetesis Book

zetesis is an answer-set solver built around the reduct. Its libraries separate
the logical question—whether an interpretation is an answer set—from the
candidate generation, source joins, Boolean transforms and parallel execution
used to answer it. The native grounding and solving paths do not invoke clingo.

This book has three parts:

- **[Solver design and architecture](architecture/tour.md)** follows one program
  through the solver, then develops the semantic and execution contracts. It
  assumes familiarity with ASP rules, default negation and interpretations.
- **[The Rust library programmer's manual](rust/libraries.md)** explains the
  existing library boundaries, ownership, resource limits and results. It assumes
  Rust fluency; it does not assume familiarity with clingo's API.
- **[The Lean proof library](lean/foundations.md)** introduces the mathematical
  definitions, reusable laws and their hypotheses. Its explanations assume ASP
  and basic mathematical proof reading; the worked argument does not require
  tactic fluency. Changing or extending proofs does require Lean.

The Rust examples teach zetesis's preparation, solving and result APIs. The
themelios library has a separate documentation scope for its own APIs and solver
integration. This book explains shared source/analysis types where they affect
the guarantees of a zetesis operation.

The mathematical library is a deliverable in its own right. It can be imported
without running the Rust solver. Its checked theorems establish the stated
mathematics; they do not yet constitute end-to-end verification of the Rust
implementation, source compiler or WGSL shaders.

The architecture uses **answer set** for a verified semantic interpretation and
**world view** for a program's complete family of answer sets. The Rust
`AnswerSet` records checked membership; optional bounded `WorldView` collection
also requires exhaustive original enumeration and complete capture. The
[vocabulary map](vocabulary.md) relates these guarantees to the execution
representations. An incomplete stream and a display projection are not a world view.

The [neuromorphic appendix](appendices/neuromorphic.md) applies these contracts
to a proposed event backend for Loihi 2 and SpiNNaker2, with explicit proof and
hardware qualification obligations.

Start with the [guided tour](architecture/tour.md), or go directly to
[embedding a solve](rust/sessions.md). The examples use existing library APIs.
The book deliberately separates current capabilities from unproved
correspondences and unsupported execution combinations.
