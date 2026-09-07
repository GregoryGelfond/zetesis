# Program input and output contracts

The modeler's convention gives `#defined` and `#show` a useful interface meaning:
intended input signatures and observable outputs. zetesis should retain that
intent in analysis over the typed themelios `Program`, before grounding or
solving. The public vocabulary ultimately belongs with the shared themelios
program/solve API; this note proposes no competing session framework.

There are two distinct pieces of evidence. A directive records what the author
declares. Structural analysis determines how the program actually reads and
produces that signature. Keep both, including their source provenance, so that
an intended input also produced by a rule is visible instead of silently assigned
an incorrect module role.

## Compatibility and the estate convention

Under clingo semantics, `#defined p/n.` declares a known predicate signature for
undefined-predicate diagnostics. It neither supplies facts nor makes atoms open
choices or mutable externals. `#show` selects observations; it does not remove
hidden stable models. Those semantics remain unchanged in zetesis.

Signature identity includes classical sign: `#defined p/n` and `#defined -p/n`
are different declarations, and `#show p/n` and `#show -p/n` select different
signed predicates within the atom channel. Declaring or showing one polarity neither introduces its opposite nor
changes coherence. Directly negated output constructors remain displayed terms,
not new logical producers. The source adapter's coherence constraints still apply
before any display selection or objective evaluation.

As an estate convention, `#defined` can additionally identify intended inputs
for a program contract. An analyzed interface should distinguish this declared
intent from a validated input-only role. Some programs use `#defined` for an
internally produced predicate, and some inputs have no declaration. Neither case
justifies changing their answer sets. An explicit future strict interface policy
can diagnose contract violations; default compatible solving must still interpret
the original source semantics.

An input signature permits an empty supplied relation unless a separate contract
requires otherwise. It establishes no argument type, finite range, key, totality,
required fact or nonemptiness by itself. These are additional assumptions that
must have their own declarations or checked evidence.

The implemented output layer includes signature/empty selections and bounded
conditional term observations. Observation expressions and their scopes are
richer than a set of predicate signatures. Term-only shows retain default atom
output; only a signature or empty show switches to explicit atom selection.
Each atom and term channel deduplicates internally, and their combined display
retains cross-channel duplicate symbols. Full-model multiplicities remain separate.

## Interface evidence for planning

A future interface descriptor should retain:

- The exact source program and context identity, with declaration provenance.
- Intended input signatures and analyzed read/producer occurrences, including
  unresolved roles and signatures that are both supplied and derived.
- Output selections or expressions, their scopes and their dependencies.
- Explicit assumptions about input values and completeness, separate from facts
  inferred for one supplied dataset.
- The task's observation, enumeration, projection and objective contract.

An open reusable encoding may later receive additional input facts. Its current
supplied values are not an upper bound for all future datasets. Analyze an
identified closed invocation for concrete finite domains, or retain unknown/open
input domains until a trusted range assumption bounds them. Changes to supplied
input, active parts or callback context invalidate the dependent evidence.

The input contract can initialize [domain analysis](domain-analysis.md); the
output contract initializes [demand analysis](demand-and-magic-sets.md). Positive
producer and signed dependency information links them. Interface assumptions
also provide useful hypotheses for splitting, conditional caches and residual
tasks. No pair of directive lists alone establishes a splitting set or guarantees
that a hidden component has an extension.

For each valid supplied input `I`, the desired external behavior can be described
as the observations of stable models of `P union I`. A query mode may compare the
set of observations. Default full enumeration additionally accounts for distinct
hidden extensions and multiplicities; optimization adds exact objective vectors
and tie requirements. These are different equivalence obligations and must not
share an undifferentiated correctness flag.

This interface view has a close connection to the existing ASP notion of
external behavior and external equivalence. Anthem's user guides explicitly
identify inputs, outputs and input assumptions; its input predicates have an
input-only restriction. Those results provide a relevant formal target under
their hypotheses, not an automatic theorem about arbitrary `#defined` usage.
[Anthem user guides](https://docs.potassco.org/anthem/guide.html),
[external equivalence](https://docs.potassco.org/anthem-cx/reference/external-equivalence/)

## Current boundary

zetesis already retains source-qualified `#defined`, signature/empty `#show`
metadata and bounded executable observation templates. A separate domain crate
consumes themelios `Program` directly; it does
not infer an open input relation merely from `#defined`. The reusable input/output
descriptor, strict contract validation, demand planner and public typed solve
adapter remain planned work. Their implementation should reuse themelios types
and preserve the same program identity rather than reparsing rendered source.
