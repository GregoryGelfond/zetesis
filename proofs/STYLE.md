# A readable mathematical proof library

zetesis's Lean package is a mathematical library in its own right. Its definitions
and laws are reusable without building or invoking the Rust solver. The intended
reader understands logic programming and reduct semantics; Lean expertise should
not be required to understand a theorem's purpose or its major proof obligations.
Kernel checking remains required for every formal proof.

## Basis and implementation choice

Lamport's [How to Write a 21st Century Proof](https://lamport.azurewebsites.net/pubs/proof.pdf)
(2011, revised 2012), especially §§3–4, motivates hierarchical subproofs, named
assertions, explicit assumption scope and a final justification of the stated
goal. Details sit under the claim they justify, so the main argument stays visible.
This is an existing proof-writing method; adopting it here is not a novelty claim.

Our implementation uses ordinary Lean declarations and local proof blocks.
Lean's [structured tactic proofs](https://lean-lang.org/theorem_proving_in_lean4/Tactics/#structuring-tactic-proofs)
provide named intermediate facts and scoped goals; `calc` supports chains of
relations. No TLA+ syntax layer, custom proof DSL or new trusted tool is required.
This convention concerns readability and mathematical reuse in addition to the
kernel's logical correctness check.

## A theorem has three reading levels

The declaration's documentation explains the **mathematical contract**: objects,
assumptions, conclusion, scope and why the result matters. Use the same meanings
as the semantic API. Distinguish interpretations from answer sets, original truth
from frozen-reduct truth, membership from enumeration coverage, and a candidate
restriction from a rewrite of the original theory.

The proof's outer blocks give the **argument**. Name intermediate propositions by
their meaning, give each an explicit type, and prove it in its own `by` block.
Use explicit case names and local assumptions. End by supplying the original goal
from the established claims. A reader should be able to skip each block's details
without having to simulate an evolving tactic state to discover the argument.

The inner blocks supply the **justifications**. State the relevant instantiated
fact and use the named assumptions or earlier laws that establish it. Introduce
witnesses from proved existence, keep case assumptions inside their case, and
state induction hypotheses at the mathematical boundary. Arithmetic normalization
and definitional simplification can remain compact at the leaves.

This is a readability standard, not a required amount of indentation. A short
`exact` application to a named law may be the clearest complete proof. Do not
inflate such proofs with empty structure or give every local assertion a public
theorem name. Use stable semantic names in Lean; optional hierarchical labels in
explanatory prose are navigation aids, not formal dependencies.

## Local proof practice

- State the proposition in `have claim : Proposition := by ...`; use `show` or
  a typed final assertion when the goal would otherwise be implicit to a reader.
- Split equivalences into named soundness/completeness directions when the two
  arguments differ. Record the coverage premise required by completeness.
- Use `suffices` only with a visible justification that the reduced obligation
  establishes the original goal. Use `calc` when transitivity is the argument.
- Keep semantic dependencies explicit. Prefer direct theorem applications and
  localized simplification; a large unqualified automation call should not hide
  the central reasoning. Automation still produces kernel-checked proof terms.
- Close a block with the exact claimed conclusion. A contradiction is explained
  by the two incompatible named facts, not merely described as impossible.
- Move a fact into the public library when it has a reusable mathematical
  contract. Keep one-off glue local. Do not couple a semantic theorem to a CLI
  option name, queue index, buffer layout or incidental Rust control-flow branch.

A scoped local fact inherits Lean's available context. Explicit naming improves
reviewability; it is not a mechanically enforced minimal-dependency discipline.
The current axiom audit checks transitive axioms, not human explanation quality
or exact lists of theorem-to-theorem dependencies.

## Library organization

Use the existing modules as independently importable mathematical capabilities.
The [reading guide](guide/README.md) groups them by the question they answer;
[theorems.json](theorems.json) remains the complete qualified-name/source index.
The umbrella `Zetesis` imports the complete library. A consumer may instead import
one module such as `Zetesis.Ferraris` or `Zetesis.CertifiedExecution` and receive
only that module's declared dependency closure.

Keep definitions mathematical and explicit. State finite coverage, order laws,
atom identity and admissible domains as hypotheses where needed. Do not conceal
such premises in a type-class instance or strengthen them merely to make a proof
shorter. Public theorem names and statements are part of the library contract;
a proof-body refactor preserves them, and a changed statement needs review as an
API/semantic change.

A module's introduction should identify its objects, central laws, dependency
boundary and unproved implementation correspondences. Distinguish reusable
semantics, algorithms over abstract data, representation refinements and concrete
counterexamples. Multiple layers may share a file when that aids comprehension;
a new directory hierarchy is not itself a mathematical improvement.

## First pilot and continuing migration

The first pilot is `CertifiedExecution.completed_membership_exact` in
[CertifiedExecution.lean](Zetesis/CertifiedExecution.lean). Its theorem name,
statement and assumptions are preserved. The [worked reading](guide/certified-membership.md)
explains its complete argument without assuming tactic fluency. The remainder of
the library is not claimed to have been converted to this style.

Adopt the convention for new substantial proofs and improve existing central
proofs in bounded reviewed slices. Suggested next targets are frozen-reduct
preservation, complete lazy acceptance and batched coverage. Preserve theorem
statements first; factor reusable intermediate laws only where their independent
purpose is clear. A browsable proof viewer may be useful later, but checked Lean
source and a readable guide are sufficient to start.

Changes pass the pinned clean build, strict complete axiom audit and the retained
proof-record checker. The record preserves previous sources/artifacts as needed,
updates exact declaration locations and hashes, and identifies the proof-only
scope. This does not establish a Lean-to-Rust or shader refinement. Readability,
mathematical scope and correspondence to the implementation remain separate
review obligations even when every theorem checks.
