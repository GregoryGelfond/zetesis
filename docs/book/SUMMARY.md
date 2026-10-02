# Summary

[About this book](index.md)
[Using the zetesis command](reference/commands.md)
[Benchmarking with zetesis-bench](reference/benchmarking.md)

# Part I — Solver design and architecture

- [A guided tour](architecture/tour.md)
- [Programs, answer sets, and the reduct](architecture/semantics.md)
- [From answer-set semantics to exact transforms](architecture/alignment.md)
- [Source programs and grounding](architecture/grounding.md)
- [Source preparation and grounding modules](architecture/source-pipeline.md)
- [Grounding compared with clingo](architecture/grounding-comparison.md)
- [Composing exact execution](architecture/execution.md)
- [Ownership, dependencies and parallel work](architecture/ownership.md)

# Part II — The Rust library programmer's manual

- [Getting started with the library](rust/getting-started.md)
- [Embedding an ordinary solve](rust/sessions.md)
- [Costs and shown terms](rust/costs-and-output.md)
- [Completion, resources, and output](rust/outcomes.md)
- [Library reference index](rust/libraries.md)
- [Reusing command workflows](rust/workflows.md)
- [Preparing source and interpreting analysis](rust/source.md)
- [Interpretations and retained atoms](rust/models.md)
- [Observations and host measurements](rust/measurements.md)
- [Working with finite reducts](rust/reducts.md)
- [Parallel and lazy checking](rust/parallel.md)
- [Projecting finite domains through a table](rust/finite-tables.md)

# Part III — The Lean proof library

- [Definitions and imports](lean/foundations.md)
- [Normal rules as Ferraris formulas](lean/normal-rules.md)
- [A map of the central theorems](lean/theorems.md)
- [Reading a structured proof](lean/reading.md)
- [Connecting proofs to implementations](lean/correspondence.md)
- [Refining packed membership](lean/membership.md)

# Appendices

- [Exact event execution on neuromorphic hardware](appendices/neuromorphic.md)

# Performance and testing

- [Performance results](reference/performance.md)
  - [Reusing grounding and formula preparation](reference/foundation-reuse.md)
  - [Grounding and prepared CPU closure](reference/instantiation-lazy.md)
  - [Reusing support-publication directories](reference/support-publication.md)
  - [Canonical storage and CPU worker scaling](reference/canonical-storage.md)
  - [Eager and hybrid formula grounding](reference/hybrid-grounding.md)
  - [Lending completed grounding rows](reference/grounding-row-lending.md)
  - [Worker scaling: CPU and Metal](reference/worker-scaling.md)
  - [CPU candidate-region scheduling](reference/scheduler-scaling.md)
  - [Shared-plan execution: CPU and Metal](reference/plan-execution.md)
  - [CPU and Metal execution series](reference/execution-series.md)
  - [Reduct execution: CPU and Metal](reference/reduct-execution.md)
  - [Prepared grounding: CPU and Metal](reference/prepared-metal.md)
  - [Earlier grounding measurements](reference/grounding-measurements.md)
- [Validating an implementation change](reference/validation.md)
  - [Measurement protocols](reference/measurement-protocols.md)
  - [Source revision identities](reference/source-revisions.md)
  - [Version 0.1.6 coverage](reference/coverage-0.1.6.md)
  - [Version 0.1.5 coverage](reference/coverage-0.1.5.md)

---

[Admitted language](reference/language.md)
[Language coverage obligations](reference/language-coverage.md)
[Vocabulary](vocabulary.md)
[Building the documentation](building.md)
