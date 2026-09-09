# World membership masks in lazy source joins

Status: implemented as an opt-in library source selection, with portable semantic,
resource and transport checks. The [source-mask record](../verification/source-masks-20260908/README.md)
records its exact qualification scope. Physical execution of the new selection
and runtime improvement remain unmeasured. The scope is the admitted relational
profile of immutable positive reduct rounds; general lazy Ferraris formulas
require another design.

The default source coordinator joins the union of derived world snapshots. A
join can combine rows from different worlds even when no individual world has
all of them. The device correctly rejects those combinations. A membership mask
can avoid some of that source enumeration, instance copying and upload while
leaving per-world reduct inference on the device.

## The transform

For a fixed batch of candidate occurrences and round snapshot `C`, define
`rowMask(a) = { w | a ∈ C[w] }`. Attach this mask to each source relation row.
After ordinary term unification, a partial positive join carries the intersection
of its selected row masks. An empty intersection makes that partial binding
unusable in every current world, so its remaining join extensions can be skipped.
The empty positive body starts with all current world bits, not an empty mask.
Facts and constraints without positive antecedents must remain available.

For a completed binding, a nonempty mask establishes only possible positive
enablement in this round. Source filters and frozen gates still apply. Initially,
keep all existing per-world positive and gate checks in the device; use masks
only to discard zero-mask source work. Uploading masks to schedule fewer
world/instance checks remains a future step requiring measurement and its own ABI
obligations.
The current relational filters are equality/disequality tests. Extending this
pruning to partial or fallible pure source evaluation would also need to preserve
that evaluator's refusal policy; logical consequence equivalence
alone would not establish that stronger contract.

Masks must describe the same immutable snapshot as the whole round. They never
contain seed truth or uncommitted deltas. Refresh after each successful commit,
including new world bits for an already catalogued atom. A stale mask can miss a
newly enabled binding. Candidate occurrences, including duplicate seeds, retain
separate world bits. Program, batch, snapshot generation, word widths and tail
bits must be validated wherever a packed representation crosses a boundary.

## Coverage and catalog obligations

A zero mask authorizes discarding work only for this batch and this snapshot.
It does not establish impossibility in another candidate or a later round. For
example, a rule `h :- p,q.` has a zero positive mask in worlds containing only
`p` or only `q`; a later world containing both must still derive `h`.

The candidate cursor currently obtains its carrier independently from
`Program::gate_atoms()`. Preserve that separation and its empty-seed bootstrap.
Do not remove candidate-carrier atoms, possible-support rows or source rules
because their current execution mask is zero. Any future proposal to apply
masks to possible-support or candidate discovery needs coverage over every
relevant future interpretation, rather than this batch's snapshots.

Skipping an instance may also avoid interning its currently underived head or
gate atoms. That is safe only if the execution catalog remains a representation
of demanded identities, never a certificate of complete candidate support. Keep
all supplied seed atoms in the final projection check. Preserve existing IDs,
revisit newly enabled source instances after commit, and admit their missing
IDs before encoding them. A reduced catalog can change when a resource cap is
reached; it cannot justify an exhaustive-search claim on its own.

## Bounds and proof outline

Reserve checked mask and join-frame storage before allocation and include it in
the host payload budget. Charge mask-word work under an explicit bounded source
work policy, record it separately, and poll cancellation during long scans.
Existing source operations retain their meaning; skipped operations are saved
work, not completed-instance receipts. Do not reset quotas per chunk or round.
Interrupted scans still discard all pending deltas and publish no completed
checks. The final no-growth conclusion still requires exhaustive coverage of
every binding that can be enabled in any current world.

Using `Zetesis.LazyRounds` and the existing `Bind` relation, the proof obligations are:

1. A world belongs to a binding's mask exactly when its positive snapshot
   satisfies all bound positive antecedents.
2. Intersecting additional row masks cannot restore a world removed by a partial
   intersection; therefore zero-mask partial joins have no enabled extension.
3. Restricting an exhausted union scan to nonzero masks preserves
   `SourceCoverage` for every current world, and consequently preserves the
   existing consequence, constraint and completed-round exactness laws.
4. Packed mask construction, stride changes, refresh and zero detection refine
   those set operations. These implementation correspondences are separate
   obligations, not consequences of the abstract laws alone.

`WorldMasks.lean` establishes the abstract set and coverage claims in obligations
1–3 under its explicit scan premises. Obligation 4 remains unproved: executable
boundary tests support the implementation but do not supply a Rust/WGSL refinement.

## Measurement controls

Compare mask-off and mask-on builds over identical programs, ordered seed
batches, immutable round inputs and configured limits. Check complete symbolic
closures, rejection reasons, candidate exhaustion and partial/error accounting.
Include a negative control that incorrectly reuses a mask after a new world
membership appears, plus a later-candidate control that exposes carrier pruning.

Use sparse world overlap, disjoint-world Cartesian joins and dense/full overlap,
including one world, irregular batches, repeated rows and duplicate candidates.
Measure end-to-end CPU/Rayon/Metal time, peak host memory, source work, mask-word
work, offered instances, dispatches and transfers. Dense overlap and small
batches are necessary overhead controls. Resource-boundary tests must check the
new accounting exactly; optimized and baseline executions need not stop at the
same point after their work differs. Keep automatic routing unchanged until the
matched results justify a policy change.
