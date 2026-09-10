# Summary

[About this book](index.md)

# Part I — Solver design and architecture

- [A guided tour](architecture/tour.md)
- [Programs, answer sets, and the reduct](architecture/semantics.md)
- [From answer-set semantics to exact transforms](architecture/alignment.md)
- [Source programs and grounding](architecture/grounding.md)
- [Grounding compared with clingo](architecture/grounding-comparison.md)
- [Composing exact execution](architecture/execution.md)

# Part II — The Rust library programmer's manual

- [Choosing a library boundary](rust/libraries.md)
- [Preparing source and interpreting analysis](rust/source.md)
- [Embedding an ordinary solve](rust/sessions.md)
- [Working with finite reducts](rust/reducts.md)
- [Parallel and lazy checking](rust/parallel.md)
- [Completion, resources, and output](rust/outcomes.md)

# Part III — The Lean proof library

- [Definitions and imports](lean/foundations.md)
- [Normal rules as Ferraris formulas](lean/normal-rules.md)
- [A map of the central theorems](lean/theorems.md)
- [Reading a structured proof](lean/reading.md)
- [Connecting proofs to implementations](lean/correspondence.md)
- [Refining packed membership](lean/membership.md)

# Appendices

- [Exact event execution on neuromorphic hardware](appendices/neuromorphic.md)

---

[Admitted language](reference/language.md)
[Language coverage obligations](reference/language-coverage.md)
[Validating an implementation change](reference/validation.md)
[Vocabulary](vocabulary.md)
[Building the documentation](building.md)
