# Selectable aggregate semantics after 1.0

Status: accepted direction for a post-1.0 feature; not implemented. Version 1.0
continues to target the admitted clingo/Ferraris language. The reduct remains the
semantic foundation under either interpretation.

The proposed user interface is `--aggregate-semantics clingo` (the default) or
`--aggregate-semantics gelfond-zhang`, backed by a typed library configuration.
These spellings are provisional. The choice changes program meaning, so it must
be explicit in admitted-program identity, execution plans, caches, stable-model
receipts, JSON, statistics and qualification reports. It is not a backend or
performance setting, and backend selection must never choose semantics.

## Definitions and prior implementation

Gelfond and Zhang's 2014 paper defines an aggregate reduct in §2.2: discard rules
whose aggregates are false in the candidate, and replace remaining aggregates
by their candidate-selected supporting literals. Answer sets are answer sets of
that resulting program. This supplies a direct reduct-based reference definition.
Its original language and variable scope differ from clingo's; supporting a GZ
interpretation of clingo-style aggregates is not automatically implementing the
entire original A-log language. [Original paper, §§2.1–2.2](https://arxiv.org/pdf/1405.3637).

Alviano and Leone give two polynomial, faithful, modular compilations from
G-stable to F-stable semantics, including a compilation producing stratified
aggregates. Their 2015 paper reports a prototype using WASP. This verifies a
historical implementation route; it does not establish that current WASP has a
built-in command-line switch. Its adapted syntax and reduct assumptions need
checking before using the translation as an oracle for zetesis.
[Compilation paper, §§2, 4, 6](https://arxiv.org/pdf/1507.03922).

Cabalar, Fandinno, Schaub and Schellhorn provide a strongly equivalent
propositional-formula account of GZ aggregates, including generalized conditions.
That is a promising route to investigate for reusing zetesis's formula/reduct
execution primitives. It does not by itself establish a correspondence with
zetesis's current aggregate lowering.
[Formula account](https://www.cs.uni-potsdam.de/wv/publications/DBLP_journals/ai/CabalarFSS19.pdf).

## Implementation obligations

1. Specify the supported source language separately from aggregate interpretation:
   local/global binding, tuple identity, duplicate keys, multiple supports,
   default-negated conditions, signed weights, extrema, assignments and head
   aggregates. Preserve themelios's documented source validation. Refuse contexts
   whose GZ meaning has not been defined instead of borrowing Ferraris behavior.
2. Preserve the complete aggregate domain and original element conditions until
   the chosen semantics is lowered. A count or selected numeric value alone is
   insufficient to reconstruct the candidate-dependent reduct. Audit eager and
   lazy carrier completion independently; partial joins do not establish that
   every relevant support has been seen.
3. Give the direct GZ reduct a small independent executable specification and
   Lean definition. Prove original truth and frozen-reduct correspondence for any
   compilation into the existing formula engine, then derive stable membership.
   State auxiliary-atom projection and full-model enumeration obligations when
   a translation introduces helpers.
4. Audit candidate pruning, class certificates, source rewrites and aggregate
   dependency plans for the selected semantics. Prove applicability separately;
   a Ferraris preservation theorem does not transfer merely because ordinary
   aggregate truth agrees. Keep unsupported certificates disabled explicitly.
5. Add examples where the two meanings differ, as well as a proved coincidence
   fragment. Compare complete answer sets with the direct specification and a
   separately validated compilation. Untranslated clingo output is only an
   oracle for the clingo mode. Include recursive, disjunctive, negated, empty-set,
   multiple-support and scope-boundary cases as each context is admitted.
6. Qualify scalar, Rayon and physical GPU execution against the same selected
   semantics, including exact residual completion, limits and interruption.
   Preserve default-mode behavior and existing full-model/cost regressions.

This future semantic capability requires original aggregate structure and
semantic assumptions to remain inspectable rather than conflating them with
an execution shortcut.
