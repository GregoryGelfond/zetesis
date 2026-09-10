# Reading a structured proof

A substantial proof should be readable at three levels: its mathematical
contract, the principal argument, and each local justification. The library
uses ordinary Lean declarations, named `have` blocks and explicit conclusions
to make those levels visible. This follows the hierarchical proof-writing
approach described by Leslie Lamport in
[How to Write a 21st Century Proof](https://lamport.azurewebsites.net/pubs/proof.pdf).
There is no custom proof language layered over Lean.

Knuth's [literate programming](https://cs.stanford.edu/~knuth/lp.html) addresses
the human reader; Dijkstra's
[constructive approach to correctness](https://www.cs.utexas.edu/~EWD/transcriptions/EWD02xx/EWD209.html)
develops the proof with the program. Here, English statements and proof outlines
let an ASP specialist inspect the claim and assumptions. Lean checks the formal
argument. Review must also establish that the prose and formal statement agree.

Consider `CertifiedExecution.completed_membership_exact`. It connects a sound
optional certificate with exact completion of any residual membership work.
Fix the original theory `T` and candidate `M` throughout the argument.

## The contract

The preliminary verdict has four possibilities:

| Verdict | Meaning supplied by its soundness premise |
| --- | --- |
| `stable` | `M` is an answer set of `T` |
| `notModel` | `M` does not satisfy the original `T` |
| `residual` | No membership conclusion; exact work remains |
| `stopped` | No completed membership answer |

The theorem assumes that the verdict is sound, that a supplied exact residual
result agrees with stability whenever it is present, and that combining the
verdict with that result completed with a Boolean answer.

It concludes that this answer is true **if and only if** `M` is an answer set of
the same original theory `T`. The residual oracle's exactness is a premise; this
composition theorem does not build that oracle or prove that it terminates.

## The argument

1. **Certified case.** Completion returns true, and verdict soundness supplies
   stability. Both sides of the desired equivalence hold.
2. **Original-model rejection.** Completion returns false. Soundness says that
   `M` fails the original theory, whereas every stable interpretation satisfies
   it. Both sides are false.
3. **Residual case.** Completion passes through the supplied exact result. The
   completed-result premise makes that result available, so its exactness
   premise gives the required equivalence directly.
4. **Stopped case.** Completion is `none`, contradicting the assumption that a
   Boolean result was returned. This case is impossible under the theorem's
   hypotheses.

Those cases exhaust the verdict type. The final case split combines the local
claims into the original conclusion. Here is the proof directly included from
the Lean module:

```lean
{{#include ../../../proofs/Zetesis/CertifiedExecution.lean:66:98}}
```

The explicit types on local claims let a reader skip tactical details without
losing the argument. Small proofs can remain one-line applications; readability
does not require ceremonial nesting. Names describe propositions, and the
final term establishes the stated goal.

The library is not uniformly written in this expanded style. New substantial
arguments and refinements should preserve named hypotheses and readable proof
structure while keeping definitions and theorem statements stable when only the
proof body changes. Kernel checking and human comprehensibility answer different
questions.
