# Packed closure of a normal reduct

[`PackedClosure.lean`](../Zetesis/PackedClosure.lean) connects executable packed
operations to the existing [finite closure algorithm](finite-closure.md). The
namespace is `Zetesis.Refinement.PackedClosure`. Its normalized rules have already
passed their ground filters, and its atom coordinates are `Fin size`. Both
conditions are explicit structural boundaries; no source grounding or admission
claim follows from this module.

The new representation uses 32-bit words, matching the width used by the static
normal CPU checker. Frozen gate selection occurs once. Consequence rules then
read and immediately update the current packed truth in source-list order. A
later rule can use a head inserted earlier in the same scan. Only a fresh
insertion raises the scan's changed flag.

## The representation is maintained by actual word operations

`Packing.read` uses a shifted one-bit mask. `Packing.insert` ORs that mask into
the corresponding word. `insert_exact` proves that this write changes only its
selected bit and preserves storage length. Division and remainder identify both
the containing word and the bit inside it.

`Packing.Represents` records the exact word count and membership at every
natural-number coordinate. Its input list contains bounded coordinates, so the
same statement includes zero raw padding. The missing-word accessor's default
does not hide a memory-shape premise: `coordinate_stored` proves that every
admitted read or write addresses an existing word.

Initialization creates exactly the required number of zero 32-bit words, then
folds the same 32-bit insertion over the seed coordinates in their input order.
`zero_represents`, `insert_all_represents` and `encode_represents` derive the
initial invariant from those operations. Repeated seed coordinates need no
special assumption. The module reuses the word accessor, word-count definition
and raw 32-bit interpretation from
[packed interpretations](packed-interpretations.md); no width conversion is
part of this closure algorithm.

## Selection and stopping are proved, not assumed

`enabled_refines` relates both frozen gate polarities to the list algorithm.
`selection_refines` then proves that removing disabled rules and constraints
preserves each sequential consequence scan. It retains the selected rules'
order. Constraints do not insert heads; their final checking is outside this
module.

`step_refines` establishes two properties together: the updated words represent
the reference step, and a false changed flag means that the reference list did
not change. `scan_refines` lifts both properties through an entire sequential
scan. Its reverse stopping argument uses inflationarity: a later rule cannot
undo an earlier fresh insertion. Thus the flag cannot falsely certify an
unchanged full scan.

`iterate_refines` reconstructs a reference execution from every completed packed
execution. `iterate_completes` proves the other direction: reference completion
is not lost by changing the representation. `selection_iteration` supplies the
same round-by-round correspondence after frozen preselection.

The public `close` operation combines initial encoding, selection, and packed
iteration from empty positive truth. `close_completes` proves that the number of
listed heads plus one is sufficient fuel. `close_exact` proves that any
completed result denotes `Semantics.Gamma` for the original normalized rules and
seed. It also establishes exact word count and zero raw padding. These conclusions
contain no assumed equality between the packed answer and the semantic closure.

Fuel counts complete scans, rather than individual operations. With `h` listed
heads, at most `h + 1` scans suffice, including the final unchanged scan.
Within a scan, each selected rule tests its head and at most its listed positive
body atoms, then performs at most one insertion. These are logical operation
bounds, not an allocation or timing theorem. Lean list indexing and updates are
not constant-time array operations; this model makes no Rust performance claim.

## What this establishes for an implementation

The corresponding Rust code is `crates/zetesis-cpu/src/static_oracle.rs`:
`contains`, `insert`, `all_present`, `enabled`, frozen consequence selection in
`check_static_view`, and the sequential loop in `close`.

The proof removes assumed agreement from this authored packed algorithm. It is
not an extraction or refinement proof of those Rust functions. A concrete bridge
still must establish canonical atom identity and admitted rule coordinates,
vector initialization, access and mutation, machine arithmetic,
allocation, cancellation, work and consequence limits, and program ownership.
The Rust shortcut for an empty selected consequence list also performs no scan;
the mathematical algorithm observes one unchanged scan. Their logical closure
agrees, but this module does not equate statistics or execution traces.

Final constraint checking and gate-projection agreement are separate operations:
a least reduct closure alone is not an answer-set acceptance decision.
[Packed acceptance](packed-acceptance.md) composes those final operations with
this closure result. Candidate enumeration and delivery remain further
obligations.
