# Boolean connectives and unit thresholds

[ThresholdConnectives](../Zetesis/ThresholdConnectives.lean) relates finite
conjunctions and disjunctions to nonnegative weighted thresholds. A conjunction
of n listed conditions requires all n unit contributions. A disjunction requires
one. Conditions may be arbitrary formulas, including compound conditions.

`unitRow` attaches weight one to every listed condition. The selected sum is at
most the list's length (`masked_unit_sum_le_length`). Reaching that length is
equivalent to every condition being true (`masked_unit_sum_full`); reaching one
is equivalent to having a true condition (`masked_unit_sum_positive`). These
statements include the empty identities: conjunction is true and disjunction is
false. Repetition needs no special premise because repeated positions share
their condition's truth.

`conjunction_original` and `disjunction_original` accept an original operand list
and a normalized list with exactly the same members. Existing finite-connective
laws describe their universal requirements or existential witnesses. Membership
equivalence carries those obligations to the normalized list; the unit-sum laws
and `ThresholdReduct.original` then identify the threshold's original truth.
This justifies sorting or deduplicating Boolean operands when the caller proves
membership preservation. The conjunction bound uses the normalized length.
It does not justify deduplicating contributions of a general weighted aggregate.

The [prefix laws](threshold-prefixes.md) can then transport necessary bounds
between related rows. Their prefix premise compares complete weight/condition
pairs. Sharing condition IDs alone does not establish that a Boolean unit row
is equal to, or a prefix of, an arbitrary weighted row. Equal rows use the empty
suffix case; missing intermediate threshold guards do not invalidate the sum
inequality itself.

These are original-satisfaction laws. They support an original-candidate bridge;
they neither change frozen propagation nor establish reduct equivalence for an
executable rewrite. Recognizing normalized membership and row relationships,
relating canonical IDs to formula meanings, finite-width arithmetic, scheduling,
resource accounting and concrete Rust correspondence remain separate obligations.
