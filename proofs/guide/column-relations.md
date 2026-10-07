# From typed tuples to equality columns

[ColumnRelations](../Zetesis/ColumnRelations.lean) specifies the representation
and selection contract for one immutable relation snapshot. The original tuple
is the semantic object. Columns and equality IDs are an execution view of it.
The declarations and their hypotheses are authoritative.

## Objects and assumptions

A `Dictionary` has finitely many IDs, a total decoder for valid IDs and a partial
encoder for complete logical values. Construction must establish both round
trips: encoding a decoded ID returns that ID, and decoding a successful lookup
returns the original value. Absence has no ID. IDs convey equality only; their
numeric order says nothing about ASP term order or arithmetic.

`Columns dictionary rows arity` assigns one valid dictionary ID to each column
and row. Every column shares the same finite row domain. Reconstruction reads
all arguments at one row position. The explicit row count remains meaningful
when arity is zero: no columns alone cannot distinguish an empty relation from
an existing empty tuple.

A query is a conjunction of whole-value equalities at checked argument positions.
Its input sequence may be all rows or a previously justified posting. The laws
establish completeness relative to that input; they do not establish that an
upstream posting or grounder supplied every required row.

## The argument

1. `encoding_exact` derives exact value/ID lookup from the two round trips.
   `identifier_equality` applies encoding to equal decoded values, proving that
   two IDs denote the same value only when they are equal.
2. `reconstruction_exact` assumes each original cell was encoded at its matching
   column and row. Decoding that cell recovers the original value. Extending this
   argument over every argument position recovers the complete original table.
3. `accepts_exact` replaces each ID comparison with its typed equality.
   `selection_exact` combines that result with ordinary list-filter membership.
   `selection_subsequence` and `selection_increasing` reuse the library's filter
   laws to retain row order, and uniqueness when the input positions increase.
4. `full_matches_preserved` keeps the original total Boolean matcher. Assume
   every full match satisfies the prefilter equalities. A matching row passes
   both filters; every other row is still rejected by the matcher. Consequently,
   the complete ordered sequence of matched row occurrences is unchanged.
5. `applicability_exact` requires the selection's exact snapshot owner and
   in-range row positions. Equal dimensions or equal tuple values do not provide
   the owner premise. Owner tokens denote fixed snapshots throughout the model.

## Preserve a source posting

`source_posting_exact` relates a single equality lookup in the encoded view to
filtering the original typed rows at the same argument position. Exact cell
encoding first reconstructs the source table. `accepts_exact` then reduces the
one-element conjunction to that original argument equality. Both paths filter
the same row sequence, so their entire posting lists are equal.

This is stronger than retaining every successful full match: it keeps every row
presented to the matcher, including eventual rejections. Equal posting lists
also have equal lengths. A fixed shortest-posting chooser therefore receives
the same alternatives in the same order. Its eligibility, tie handling and
source-to-local position mapping remain implementation obligations. Replacing
one posting by an intersection could skip additional matcher visits; this law
does not authorize that change or establish its error behavior.

## Decode an ordered mask

`RowMask` supplies one Boolean for each finite row position. `decodeRows` scans
those positions in canonical increasing order and retains the true bits.
`row_mask_roundtrip` assumes that these bits describe exactly an increasing
selection. The decoded and original lists have the same members and no repeated
positions, hence are permutations. Since both increase, they are equal.

The strict-order premise matters. A bit cannot record two copies of one
position. Equal source tuples at different positions are separate occurrences
and retain separate bits. Empty relations and empty selections use the same
law. It proves nothing about full relation coverage unless the selected
positions already have that coverage. Matching dimensions or valid mask shape
do not establish the exact-bit or snapshot-owner premise.

The tuples `(a, 1)` and `(b, 2)` illustrate why correlation matters. The query
requiring first argument `a` and second argument `2` has no row. Separate column
membership tests would find both values; selection at a shared row finds none.
The laws also retain every input row for no equalities and reject every row
when a requested whole value is absent from the dictionary.

## Store columns at different widths

[CompactColumns](../Zetesis/CompactColumns.lean) adds checked finite-word
representations to the same dictionary and row domains. `packed_exact` shows
that successful packing decodes to the complete original column. Its premise
requires every actual identifier to fit; the number of distinct identifiers
alone is insufficient. `resize_exact` preserves existing values when the new
width represents them, and `append_prefix` retains the previous row sequence.

`encoded_cell_exact` applies that round trip to aligned relation columns.
`accepts_exact` and `selection_exact` then preserve equality checking and the
entire selected row sequence, including equal tuples at different positions.
The laws apply to 8-, 16- and 32-bit cells without assigning logical meaning to
the numerical order of their IDs. They do not prove the Rust width-selection
code, allocation accounting or the device buffer's offsets and packed lanes.

## What remains outside the proof

The matcher in `full_matches_preserved` is total and pure. The theorem does not
preserve runtime error order, binding rollback, allocation failure, cancellation
or resource-limited prefixes. Those obligations remain with the matcher and
its execution boundary. In particular, skipping a row cannot silently suppress
an otherwise required source arithmetic diagnostic.

Dictionary construction, checked integer packing, Rust borrowing and snapshot
lifetime need their own correspondence arguments. Machine mask production and
decoding must establish the abstract row-mask contract, including complete word
coverage and valid tail bits. Local row positions do not establish external atom-catalog IDs;
source/catalog mapping and any initial increasing-row invariant are separate
runtime obligations. Possible-support membership and current-world truth remain
different subjects. None of these representation laws alone proves complete
grounding, Rust/WGSL refinement or end-to-end solver verification.
