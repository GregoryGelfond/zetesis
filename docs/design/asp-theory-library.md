# A reusable ASP theory library

Status: accepted long-term adjacent direction. The existing in-repository Lean
package remains the current proof artifact. No separate repository, extraction,
dependency change or comprehensive formalization campaign is implied by this
proposal, and the broader library is not a version 1.0 prerequisite.
For a zetesis release, develop and harden the fundamentals enabling formal
verification of the solver. The larger
collection informs definitions, boundaries and attribution; it does not expand
the immediate implementation or formalization scope.

## Purpose and consumers

Formalize established ASP definitions, lemmas and theorems as a coherent,
reusable mathematical library. Zetesis should be one consumer alongside other
solver implementations, research formalizations and teaching material. The
library's concepts should speak in the logic programmer's register and preserve
the mathematical distinctions that make the results true.

Organize foundational semantics independently of a particular solver: programs,
formulas, interpretations, reducts, satisfaction and stable models. Keep exact
language profiles visible, including normal versus disjunctive programs, finite
versus unrestricted domains, strong versus default negation, and differing
aggregate semantics. Do not export a theorem about one profile under a name that
claims all of ASP.

Candidate areas include correspondence between semantic definitions, ordinary
and strong equivalence, semantics-preserving transformations, splitting and
modularity, program-class properties, aggregate semantics, consequence reasoning
and algorithm correctness. These are research and curation topics, not claims
that the current package has formalized each area.

## Layering and proof quality

Use a dependency structure in which general definitions support mathematical
results, those results support abstract algorithms, and explicit refinement
layers connect algorithms to executable representations. Packed atom identifiers,
machine arithmetic, Rust traversal state and GPU schedules belong to the latter
layers. A solver-specific representation must not become a hidden premise of a
general semantic theorem.

For each literature result, record its source, published statement and exact
assumptions. Explain any strengthening, weakening, corrected premise or changed
notation in the formal statement. Distinguish a formalization of an existing
result from a new theorem. Shared definitions and bridge theorems should let
results compose instead of creating incompatible copies of stable-model
semantics for different papers.

Keep structured explanations next to checked proofs, with a discoverable index
of definitions, statements and dependencies. Useful counterexamples should show
why substantive premises cannot simply be dropped. Exact proof statements and
audited assumptions are the evidence; the number of declarations is only an
inventory. Unfinished results should be documented as open work, not represented
as admitted project axioms or proof holes in the qualified library.

## Development direction

Begin with results already needed to explain and verify the solver. Generalize
them when the mathematical interface is clear and add small reusable lemmas
when they remove duplication. Periodically review dependency structure,
attribution, comprehensibility and applicability beyond zetesis. Preserve the
pinned toolchain and existing proof-record discipline until a separately
reviewed dependency or extraction decision is made.

The [verified-solver direction](verified-solver.md) has a related but different
obligation: connecting formal semantics to actual executable behavior. A rich
ASP theorem library can help discharge that obligation; it does not discharge
it merely by existing. Both efforts should make their trusted components,
limitations and completed correspondences explicit.
