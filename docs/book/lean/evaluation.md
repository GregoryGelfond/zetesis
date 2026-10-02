# Refining an evaluator step

The [evaluator package](https://github.com/GregoryGelfond/zetesis/tree/main/refinement/evaluation)
extends concrete refinement from packed membership to one step of the Rust
formula evaluator. Its generated body contains the actual iterator, control
checks, node cases and output append. The proof supplies no assumption that
those operations return the right result.

## What one step establishes

Assume that the packed interpretation has enough storage, the output prefix and
node iterator agree on their position, and every child read refers to the
existing prefix. A supplied mask must contain the current position. If another
node remains, the current control observations allow progress and work remains,
the step appends exactly:

```text
nodeTruth(candidate, previousValues, currentNode)
    AND suppliedMask[currentPosition]    // if a mask is present
```

The previous prefix is unchanged. The node position and work counter each
advance by one; their machine-integer bounds follow from the represented slice
and work limit. The proof follows the five node cases: atom, false, conjunction,
disjunction and implication. It preserves their short circuits.

A cancellation or deadline observation stops before node evaluation. Either
precedes the work-limit check. These stopped steps preserve the output and work
record. Exhausting the iterator completes without polling or charging work.

The proof is organized into independently checked arguments:

| Module | Argument |
| --- | --- |
| `Membership` | An actual packed query returns the declared atom's bit |
| `Iteration` | Actual slice/vector operations respect their finite bounds |
| `Control` | The actual poll and tick preserve precedence and work accounting |
| `Step` | The generated node branches append exactly the supplied masked truth |
| `Progress` | Structural and control premises establish the complete single step |

The [package README](https://github.com/GregoryGelfond/zetesis/blob/main/refinement/evaluation/README.md)
links the checked source, trust boundary and reproduction commands. A recorded
local-name adjustment avoids a generator namespace collision; it changes no
operation or Rust source. Exported extraction metadata uses a portable destination
path. Both transformations are recorded separately.

## What remains

The mask above is supplied data. Proving that it contains the original
candidate's truth, that the prior prefix is semantically correct and that all
objects belong to the same theory remains part of whole-evaluation refinement.
The mathematical library already proves the corresponding frozen-reduct laws;
connecting the concrete representation to them is a further obligation.

Cancellation is also a precise boundary. Each atomic token here supplies the
value observed at one read site in one invocation. It does not represent shared
mutable storage or constrain a later observation. The generated whole loop must
not be treated as a verified concurrent loop by repeatedly reusing those tokens.
A suitable observation-history model and its runtime correspondence remain open.

The result uses Aeneas's vector and scalar models and explicit external library
models. It does not verify allocation, reference counting, timers, machine code
or GPU execution. The pinned extraction package uses Lean 4.31.0; the general
ASP library uses Lean 4.33.1. There is no checked cross-version composition yet.
The [correctness plan](correctness.md) keeps these obligations distinct from
answer-set soundness and complete enumeration.
