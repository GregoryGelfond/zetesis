# Language closure: first implementation slices

The starting checkpoint is `53f98ac`, with compiled release source
`0b72472`, recorded local gates, physical Metal qualification and frozen
measurements. The [language-closure plan](language-closure-tranche.md) governs
this work. Focused source evidence is integrated; the local portable/oracle
checks, release comparisons, both coverage floors and 25 frozen physical Metal
tests pass. The [checkpoint record](../verification/language-closure-tranche-20260908/README.md)
retains the qualification scope. This is not full language closure.

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

The [count-head slice](../verification/count-head-activity-20260908/README.md)
passes 35 original clingo sources with 67 full models. Its separate permission
and activity maps preserve both tuple/atom alias directions. The
[objective slice](../verification/objective-forwarding-20260908/README.md)
admits 38 new originals with 69 full model/cost records and retains ten explicit
mixed-extrema refusal controls. A discovered pre-existing direct-observer
cost-layout defect now receives a bounded refusal. That is a remaining zetesis
implementation gap, not a themelios rejection or a modeling-error claim.

Combined writer-free prepared sessions exercise six original programs with
independent aliased count heads and forwarded objective producers, across
pruning on/off, one/four completion workers and batches of one/eight. All 48
sessions exhaust the relevant search and retain the expected full model/cost
records, including all optimal ties where objectives remain present. Forwarded
empty extrema preserve absent costs rather than inventing a zero priority.
Assertions require actual pruning and batched completion work. Sixteen further
sessions stop under scratch or candidate ceilings, retain pending work or an
unproved incumbent, and make no optimum or exhaustive-coverage claim. The
five corresponding objective-dependent count-head cases remain located
refusals. Their clingo behavior is retained separately as gap evidence rather
than silently broadening the successful composition claim.

Further concrete follow-ons are nonnumeric literal objective weights, broader
objective presence carriers, and objective-relevant function heads. The first
two are recorded with original references in the objective evidence; they do
not disappear when an equivalent variable-weight profile is admitted. The
Lean library adds nine `CountHeadActivity` and six `ObjectiveTransport` laws,
with a clean 834-theorem/74-module build and full axiom audit. These semantic
laws retain their explicit compiler-correspondence limitations.

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
