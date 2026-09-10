# Domain filtering preserves compatible answer-set completions

[DomainContraction](../Zetesis/DomainContraction.lean) connects two finite
constraint filters to answer sets of an unchanged Ferraris theory. It is a
candidate restriction, not a replacement for reduct checking or a source rewrite.

## Objects and assumptions

Fix an original theory, an activation condition, and a total assignment reading
for recognized mandatory finite-domain groups. `Compatible` means an answer set
of that theory whose reading extends the supplied partial assignment, fits the
current domains, and satisfies activation. `Ferraris.Stable` supplies answer-set
membership through original satisfaction and reduct minimality.

`Recognizes` is the semantic premise behind source recognition: every active
answer set must satisfy the proposed assignment constraints. The runtime must
justify its reading of exactly-one groups and its activation condition. A shape
check or a classically consistent assignment cannot supply this premise.

`contract` filters each finite domain using the same input snapshot. `Preserves`
requires the filter to retain every fitting solution of its assignment constraint.
It is proved for two concrete operations below; an additional operation needs
its own argument.

## The argument

1. `singleton_filter_preserves` concerns distinct participating groups. If one
   group's domain contains only `v`, a fitting assignment must choose `v` there.
   Distinctness forbids `v` at every other participating group. Removing that
   value there therefore retains every fitting distinct assignment. Groups
   outside the constraint are unchanged.
2. `remainder_lower_bound` composes the existing `IntegerEnvelopes` product and
   finite-sum laws. Each coefficient selects its appropriate lower product
   endpoint, including negative coefficients. Their sum bounds the true
   remainder even when variables repeat or are correlated. Sound domain
   endpoints are a separate premise.
3. `affine_filter_preserves` replaces an affine comparison's true remainder by
   that lower bound. A satisfied upper comparison remains satisfied. Thus a
   value failing this necessary bound cannot belong to a fitting solution.
   The argument uses mathematical integers, without division or endpoint rounding.
4. `compatible_contraction` proves both directions. A narrowed compatible answer
   set fits the original domains because filtering only removes values. An
   original compatible answer set satisfies the recognized constraints; filter
   soundness retains each value in its reading. The same answer set, theory,
   activation and partial assignment remain throughout.
5. `bounded_filters_preserve_completions` composes that argument over a finite
   list prefix. Its budget counts completed filter applications, not internal
   work or convergence. `empty_domain_excludes_completions` rules out the
   resulting certified branch if a mandatory group's domain is empty.

For distinct groups with domains `{1}` and `{1, 2}`, the singleton filter removes
`1` from the second group. For `2X + Y ≤ 7` with `Y` in `{3, 4}`, the lower
remainder is `3`; it excludes `X = 3` but retains `X = 2`. Retaining `X = 2`
does not assert that every remaining choice of `Y` satisfies the comparison.
The complete constraint and reduct checks retain their roles.

## What remains outside the proof

No source recognizer, assignment-to-atom correspondence, fixed-point scheduler
or finite-width arithmetic implementation is verified here. An interrupted
filter cannot claim a completed prefix unless its publication boundary actually
establishes one. Empty-domain rejection concerns the active certified branch;
other branches may still have answer sets.

These laws directly justify post-admission candidate filtering. Pre-ground use
also needs definedness of skipped evaluations or prior preservation of the
original source arithmetic/error boundary. Answer-set retention alone cannot
justify suppressing a source error. Nonempty domains or a filtering fixed point
prove neither support, minimality nor existence. The existing
[Propagation](../Zetesis/Propagation.lean) counterexamples make those distinctions
explicit. No Rust/WGSL refinement or end-to-end verification claim follows.
