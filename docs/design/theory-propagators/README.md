# Theory extension architecture investigation

2026-09-06. Theory-extension design investigation for zetesis.

- [Architecture report](theory-propagators.md): semantic profiles, candidate and
  reduct boundaries, direct source and themelios API routes, clingcon-compatible
  CP satellite roadmap, joint answers, device execution, assurance contracts.
- [Rust interface sketch](theory-propagator-api.md): non-compiling proposal for
  the shared vocabulary/private adapter boundary, not an implemented public API.
- `reviewed-inputs.json`: read-only inspected source identities, current themelios
  versus unchanged dependency pin, installed clingcon header/wrapper evidence.
- `prospective-corpus.json`: 38 scenarios, three standalone cases, and six
  encodings from the read-only themelios copy of kr-domains, with file hashes.
  These are future targets and have not been replayed for this investigation.
- `sources.md`: primary references and the limits of this research.

No Cargo, Lean, solver, GPU, or clingo/clingcon commands were run for this
investigation. No production files or sibling repositories were modified.
Existing zetesis theory admission exclusions remain in force. This directory records the reviewed design and its inspection evidence.
Its proposed API and future acceptance targets are separate from runtime qualification.
