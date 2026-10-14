# Design notes

These notes record designs whose arguments outlive a single change: the problem with its measured evidence, the
design, its correctness and preservation argument, its costs and receipts, and its tests and acceptance. Each note
states what is implemented and what is open, and lists the review items its next revision must settle. The
[manual](../book/index.md) documents supported behaviour; a note documents a design, including parts not yet built.

- [Term identity and grounding](grounding-and-interning.md): canonical term storage as the typed vocabulary that
  grounding, candidate generation and reduct checking share.
- [Fact path](fact-path.md): oracle preparation by identity and unchanged statements kept (implemented), and one
  admission behind one entry point per door (open).
- [Facts as columnar relation rows](columnar-fact-relations.md): fact blocks on the formula route (implemented), then
  analysis inputs without a statement per fact, facts as tables in the lazy route's core program, and a fixed-true fact
  set in the theory.
- [Numeric argument domains](numeric-argument-domains.md): one static analysis of integrality and intervals per
  argument, and the consumers that make the grounding of arithmetic cheaper.
- [Grounding without repeated work](grounding-without-repeated-work.md): unaffected rich producers not traversed
  (implemented), local join plans shared per snapshot (open), and a roadmap for two more repetitions.
- [Body-atom identity](body-atom-identity.md): naming matched positive body atoms from the join's matched-row record
  instead of looking them up again.
- [Grounding control cadence](grounding-control-cadence.md): observing cancellation and limits at the metered cadence
  of grounding's work account.
- [Region-worker engagement](region-worker-engagement.md): a coordinator-first parallel region walk that starts its
  workers once the walk outlasts their start (implemented), with its measurement protocol (open).
