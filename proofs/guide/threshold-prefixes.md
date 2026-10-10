# Implications between threshold prefixes

[ThresholdPrefixes](../Zetesis/ThresholdPrefixes.lean) relates a nonnegative
weighted threshold to one whose row extends it. If the extended threshold's
bound is no greater, reaching the prefix threshold reaches the extended one.
Conversely, failure of the extended threshold excludes the prefix threshold.

The prefix contains complete weight/condition pairs. Matching condition IDs
alone is insufficient. Weights and bounds are natural numbers; zero weights
and repeated conditions retain all their occurrences. Conditions may be arbitrary
formulas. Signed-bound normalization remains an implementation obligation.

`masked_sum_append` splits each selected sum into prefix and suffix sums.
Nonnegativity gives `masked_sum_prefix_le`. The ordered bounds then give
`masked_threshold_implies`. `original_implies` applies that arithmetic law to
the mask of condition truth, and `original_contrapositive` supplies the reverse
inference from failure.

`frozen_implies` separately applies the same arithmetic law to both gates of
[ThresholdReduct](../Zetesis/ThresholdReduct.lean): original condition truth and
recursively frozen condition truth. `frozen_contrapositive` follows. These laws
hold for arbitrary interpretations M and J, without assuming J is contained
in M or that M is an original model.

For example, `a+b+c >= 2` implies `a+b+c+d >= 2`. Failure of the latter excludes
the former. The reverse implication does not hold: selecting only `a` and `d`
reaches the extended threshold but not the prefix threshold. The laws therefore
justify necessary consequences; they do not permit replacing one formula by
the other.

Recognizing a prefix, relating grouped rows to their occurrences, checked
arithmetic and complete propagation are separate implementation obligations.
The laws do not require a particular row index or establish which consequences
an implementation discovers. A concrete narrowing route needs its own
correspondence. Original evaluation and reduct construction retain their
existing meanings.
