# Reading the projected-answer laws

[`ProjectedAnswers`](../Zetesis/ProjectedAnswers.lean) separates a fixed projection key from the full original model carrying it. `Same key M N` means that the two keys agree. For a fixed key this is an equivalence relation; it does not assert that `M` and `N` contain the same original atoms or have the same objective score.

`Covers key selected representatives` has two independent requirements. Every representative belongs to the selected original family, and each member of that family has a representative with the same key. The definition deliberately does not establish uniqueness. An implementation must separately ensure that it publishes each completed key only once.

`retain` models one completed identity decision: keep an existing representative,
or append the new original answer when its key is absent. `retain_covers`
preserves coverage of the consumed prefix in both cases. `retain_unique`
preserves distinct representative keys. These laws expose the two invariants
of the streaming operation without assuming a hash-table implementation.

`covered_key_image` proves that quotient coverage preserves exactly the selected family's key image. Its forward direction uses original membership of every retained representative. Its reverse direction takes the coverage witness for an original model and transports the model's key to that witness. This is not full answer-family coverage: different original models may have the same key.

`selected_property_survives` preserves any predicate already established for the selected family. A predicate can include answer-set membership, the exact source owner and an optimal cost. Selection must establish those facts first. The concrete counterexample `projection_before_selection_can_lose_optimum` shows why equal keys cannot justify choosing an arbitrary representative before minimizing: the selected representative may have a worse score than an omitted original.

The Rust correspondence uses the source-owned `PreparedProjection` domain and an explicit projected session. Its result remains a full `AnswerSet`; projection history stores positions within one fixed domain rather than another copy of the logical atoms. Ordinary enumeration and `WorldView` remain full-family doors. Conditional projection declarations must be grounded against completed source activity before any answer is selected. The mathematical key function assumes that fixed domain; these laws do not compile or certify it.

[`ObservationBindings`](../Zetesis/ObservationBindings.lean) addresses a different operation: finite values used by observation queries. Translation and reflection have one mathematical integer inverse. The bounded translation law admits that inverse only when it is inside the original interval and satisfies the complete guard. It does not prove checked machine arithmetic, multiplication inversion or the compiler's safety test. Existing `StructuralBindings` laws preserve consistent named captures, and `ProjectedConditionals` places default negation after complete anonymous witness projection.

None of these modules proves Rust allocation, key representation, typed owner authentication, cancellation, failure prefixes or work accounting. Those remain explicit source contracts and executable controls. A source-grounding success does not establish answer-set membership; a membership success does not establish complete full-family or quotient enumeration.
