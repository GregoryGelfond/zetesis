# Necessary domains and complete bindings

[`DomainBindings`](../Zetesis/DomainBindings.lean) separates two obligations in
optional eager domain guards. First, every positive argument's analyzed domain
must cover its value in each complete positive source binding. Every occurrence
of the same variable has that value, so intersecting their domains remains a
necessary condition. Unknown is unrestricted; absence of a finite certificate
is never an empty set.

Second, filtering a row before continuing a join may remove a local match that
could never finish the remaining positive body. `guarded_continuations_exact`
preserves the ordered flattened completion list if every row with any complete
result survives. The induction retains the accepted row's completions; a rejected
row must have none. Results may carry full row-occurrence traces and bindings,
so the statement preserves multiplicity and order. Applying this argument at
each finite join depth supplies the continuation-based composition obligation.
It does not assume equality of intermediate positive-match families, unlike the
stronger local premise used by `TableBindings.join_family_preserved`.

The concrete consumer is
[`formula_domains`](../../crates/zetesis-themelios/src/formula_domains.rs),
[rule guards](../../crates/zetesis-themelios/src/formula_support/queries/domains.rs)
and the existing final `Join`. The first release is optional and default off.
Its applicability check requires the exact normalized whole program and original
flat positive rule occurrences. It excludes generators, comparisons, negative
body gates, structured terms and richer producers from this consumer. Support
completion is unchanged. On this pure positive profile, induction over the
completed support construction and the analyzer's conservative transfers must
supply argument coverage; this source-to-implementation bridge remains a review
and executable-control obligation, not a theorem established by these lemmas.

The analyzed owner and original rule occurrence are checked before guards are
prepared. Borrowed argument symbols form variable meets; the existing relation
query resolves them to the completed owner's sole equality dictionary. Each
surviving row retains its position and passes through the original matcher.
Guard rejection occurs before binding copies and deeper probes. Complete theory,
atom order and provenance comparisons check this path against disabled analysis.
Unsupported profiles and global Unknown/Stopped outcomes use complete fallback.

Analysis, applicability, conversion, lookup and guard work consume the existing
cumulative formula budget. Analysis has separate finite logical populations and
bounded standard allocations; its heap is outside the named support/guard byte
ceiling. The guard lease accounts its named headers, scratch and actual vector
capacities beside retained query indices and live masks, including failed
preparation prefixes. None of these statements gives a hard allocator/RSS cap,
a caller cancellation contract or equal work cutoffs between strategies.
Arithmetic-bearing profiles keep their existing complete validation path.
