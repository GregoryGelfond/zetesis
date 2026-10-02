# Refining an evaluator step

This optional Lean package checks one step of the evaluator extracted from
`zetesis_ferraris::oracle::evaluate`. It includes the actual iterator, work check,
packed membership query, Boolean node branches and masked append. It does not
replace those operations with an assumed correct evaluator.

The reusable ASP theory remains in [`proofs`](../../proofs/README.md). This package
is a concrete implementation refinement with a separate Lean 4.31.0 toolchain.
It is not included in the semantic library's theorem count, and no checked
cross-version composition with that Lean 4.33.1 library is claimed.

## Claim and argument

With another node remaining, a represented packed interpretation, an aligned node iterator and output
prefix, valid child and optional mask indices, no observed stop and a remaining
work allowance, one generated step:

1. reads the actual next node and advances the slice and enumeration positions;
2. increments the work counter by exactly one, without overflow;
3. computes the node's Boolean value, preserving the source's short circuits;
4. appends that value conjoined with the supplied mask bit, when a mask exists;
5. preserves the previous prefix, all other work fields and position alignment.

`Progress.lean` composes the iterator, tick and node proofs. Its callers provide
structural bounds and observed control inputs, not an assumed next-node result,
tick verdict or node-evaluation result. `Step.lean` states the intermediate
composition explicitly. `Control.lean` establishes cancellation-before-deadline
precedence, followed by the work-limit check. A stop preserves the prior output
and work record. An exhausted iterator completes without polling or charging a
node.

The mask is a supplied vector. These theorems do not yet prove it was computed
from the original candidate or belongs to the same theory. Nor do they prove the
input prefix already agrees with the meaning of earlier nodes. Those are
invariants for the subsequent whole-evaluation proof.

## Observation and library boundary

An atomic input here denotes the Boolean observed at one read site during one
invocation. It is **not** an immutable model of shared Rust atomic storage.
The body polls at most once, reads cancellation at most once and optionally reads expiry
at most once. Each separately considered invocation receives its own inputs.
Reusing these tokens in the generated whole loop would repeat observations;
that is outside this proof's scope. A future loop proof must justify changing
observations and their connection to the runtime.

Only the Relaxed loads used by this poll are modeled. Other orderings return
an explicit unsupported-model result; this says nothing about the validity of
those orderings in Rust. No claim about memory ordering, timers or eventual
cancellation response follows.

The concrete Arc interface exposes a live value. Unused clock and synchronization
fields retain opaque tokens; no operations on them are modeled. The Aeneas vector
model records logical elements and checked index bounds, not allocation capacity
or allocation failure. Clear is used at `Bool`, where element removal has no
user-defined destructor. Reference counting, pointer identity, destruction and
concurrent memory remain outside the model.

The Rust compiler, Charon and Aeneas translations, and correspondence of their
library models to Rust, remain trusted boundaries. There are no project axioms,
proof holes or native proof-evaluation shortcuts. This is not a proof of the
whole evaluator, `FrozenReduct`, answer-set checker, grounder or GPU backend.

## Extraction identity

The generated types and functions come from the production Rust method without
a new harness or a Rust source change. A pinned-generator naming defect requires
one explicit normalization: the debug name of local 1 in `evaluate` changes from
`theory` to `program`, avoiding a generated binder that shadows the `theory`
namespace. Operands refer to local IDs, not this debug spelling. Restoring this
single field restores the entire parsed exported source LLBC; the operation tree,
identifiers, types, spans and embedded Rust source remain unchanged.

The source LLBC export first replaces only the extraction destination metadata
with a portable filename; it does not expose a developer path. That exported
source and its debug-name-normalized counterpart are retained. `provenance.json`
identifies both fields and the generator; `artifacts.sha256` identifies the checked files.
The regenerated Lean is unedited. This normalization is an audited translation
preprocessing step, not a verified transformation or a claim of byte-identical
raw translation. Source hashes identify the selected Rust inventory.

`Mask.lean` and `Membership.lean` reuse the packed-membership argument in the
historical [membership package](../membership/README.md), instantiated against
this extraction's types. That separate historical record remains unchanged.

## Reproduction

The [reproduction guide](REPRODUCING.md) gives the pinned tool setup, strict Lean
checks, axiom audit, extraction and exact naming-normalization commands.
`Audit.lean` covers every authored theorem in this package. The retained audit
and verification record describe the checked source; they do not qualify a
later edit. No downloaded compiler, dependency or build cache is vendored.
