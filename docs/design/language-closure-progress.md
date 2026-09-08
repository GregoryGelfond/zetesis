# Language closure: first implementation slices

The starting checkpoint is `53f98ac`, with compiled release source
`0b72472`, recorded local gates, physical Metal qualification and frozen
measurements. The [language-closure plan](language-closure-tranche.md) governs
this work. No new language extension is qualified by this progress record.

| Capability | Bounded first slice | Required result |
|---|---|---|
| Head activity | Count-head tuple/atom aliases in both directions | Separate permitted atoms from complete-tuple activity; preserve original/frozen formulas and make optional CountPlan decline aliases safely. |
| Objective dependencies | Unique acyclic positive pass-through producers | Propagate generated argument positions without treating possible support as realized values; preserve objective presence, costs and optimum ties. |
| Feature composition | Shared proof registry, public boundaries, source ledger and cross-feature qualification | Independently qualify each slice and their interaction. Keep original program/reduct identity and truthful resource outcomes. |

The objective slice initially excludes filters, repeated or constant forwarding
matches, cycles and multiple producers from its certificate. Those remain
visible follow-on obligations. The count slice preserves the existing weighted
and extremal admission profiles; it does not silently promote every head
aggregate after removing a bijection check.

An intake check also refines the Boolean-head ledger. With source
`0b72472` and clingo 5.8.2, `#false.` already completes as UNSAT in both. In
contrast, `#true.`, `{a}. #true :- a.` and `#true | a.` have respectively one,
two and one complete reference models and receive located native head-form
refusals. These tiny preliminary probes identify a remaining representation
boundary; they are not a new curated campaign or compatibility gate. A future
Boolean-head slice must cover signed forms, disjunction, original/frozen truth,
binding safety, diagnostics and budgets rather than merely drop true-headed
source rules before admission.

The themelios solve design at revision `c4d4045` has
the engine-independent `Backend` contract and capability declarations already
described by the [integration note](themelios-solve-integration.md). That draft
anticipates a native backend and keeps program input, typed outcomes, exhaustion
and proven optimum distinct. The later API-cleanup rounds should implement the
needed adapter boundaries against the actual shared API. This language tranche
does not create a second estate session framework or change the themelios pin.
