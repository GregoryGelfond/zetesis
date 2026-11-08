# One pair of ordinal extrema, many cuts

`OrdinalExtrema` studies a finite list of ranks from zero through a specified
greatest rank. A single paired fold computes its minimum and maximum. Its empty
result is `(greatest, 0)`. There is no guard parameter: later comparisons read the
same mathematical result. Repeated ranks remain separate list occurrences.

The argument reuses the existing order-selection library:

1. `paired_fold` projects the pair into the two ordinary selection folds.
2. `selected_measures` identifies those folds with `ValueExtrema.extreme` and
   its explicit empty encoding. The greatest rank is neutral for minimum; zero
   is neutral for maximum. Contributions must belong to that closed carrier.
3. `prefix_witness` and `suffix_witness` instantiate
   `ValueExtrema.selected_or_empty_predicate`. At an interior cut, the empty
   sentinel cannot witness the comparison, so a selected contribution must do so.
4. The four endpoint laws establish prefix-zero false, suffix-zero true,
   prefix-above-carrier true and suffix-above-carrier false, including empty rows.

For a carrier of size V, instantiate `greatest = V - 1`; a valid nontrivial
carrier has V at least two. An interior cut lies strictly above zero and at most
that greatest rank. The prefix predicate is `rank < cut`; the suffix predicate
is `cut ≤ rank`. The theorem requires the complete rank bound, rather than
assuming integer ranks cannot exceed an unspecified carrier.

The earlier `RankedExtrema` module supplies the separate typed-value bridge:
weak order compatibility and represented-guard cut agreement identify selected
ranks with source extrema. Its original and frozen laws apply to every tuple
mask and arbitrary inner interpretation. `ValueExtrema.witness_original` and
`witness_frozen` identify an OR of qualifying formulas with its eligible
witnesses, while `guard_frozen` preserves the outer original gate for all six
comparison connectives, including inclusive-to-strict implication for not-equal.
These results are reused; this module introduces no alternative reduct.

This is a mathematical pair fold and finite-order argument. It does not verify
that Rust assigns ranks correctly, stores one authoritative row, caches one
completed measure per pass, preserves row identity or frozen gates, propagates
partial information, handles fixed-width arithmetic or charges its operations.
Ordinary GPU formula transport now retains native cut nodes and shared rank
columns. Original evaluation uses the witness/endpoint characterization above;
frozen propagation applies the original truth gate first. Wire offsets,
fixed-width comparison, atomic observations and residual completion remain
separate executable obligations. Neither successful kernel checking nor the
absence of new axioms establishes those implementation correspondences.
