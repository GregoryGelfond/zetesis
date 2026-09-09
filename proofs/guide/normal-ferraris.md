# Normalized rules and the Ferraris reduct

The two semantic foundations describe the same answer sets on the normalized
rule fragment. [NormalFerraris](../Zetesis/NormalFerraris.lean) proves this by
translating rules into formulas and comparing their models before establishing
minimality. It does not define one notion of answer set as an alias for the other.

## Objects and translation

A normalized rule has a positive body, a list of true gates, a list of false
gates, a ground filter and an optional head. For an interpretation `M`, a true
gate requires its atom in `M`; a false gate requires its atom absent. During
reduct checking those gates remain fixed at `M`, while the positive body is
evaluated in a tested subset `J`.

The translation makes this distinction explicit:

| Normalized component | Ferraris formula |
|---|---|
| Positive body atom `a` | `a` |
| True gate `a`, including a normalized choice gate | `not not a` |
| False gate `a` | `not a` |
| Empty body | `⊥ → ⊥` |
| Head atom `h` | `h` |
| Missing head, a constraint | `⊥` |
| Rule | `body → head` |

Here `not F` means `F → ⊥`; it is not an instruction to complement a Boolean
value inside the reduct. Positive literals come first, then true gates, then
false gates. The first literal is the initial body and subsequent literals form
a left-associated conjunction. This matches the tree shape built by
`zetesis-ferraris::normal::from_ground_program`, apart from its concrete DAG
storage and atom indices.

`Semantics.Rule.filter` is a proposition about already-ground values.
`translate` selects exactly the rules for which it holds. This is a
noncomputable denotation, not an evaluator for arbitrary propositions. The Rust
`GroundProgram` boundary has no filter field: retained rules must already have
their filters discharged. `Applicable P` states that premise, and
`translate_applicable` proves that under it the filtered translation equals
`P.map rule`, preserving the input order.

## Proof structure

The proof proceeds through named correspondences:

1. `reduct_antecedent` shows that a frozen body evaluates positive atoms at `J`
   and both gate polarities at `M`. The explicit premise is `Sub J M`.
2. `reduct_rule` retains the original rule's truth at `M` as well as its frozen
   obligation at `J`. Omitting the first conjunct would incorrectly admit a
   false original implication as a reduct model.
3. `reduct_model_iff_rules` expresses normalized closure and constraints as the
   requirement that each applicable gated body entails its optional head.
4. `models_translate` equates original formula models with normalized models
   of their own frozen rules.
5. `models_frozen_translate` establishes, for `J ⊆ M`,

   ```text
   J models the Ferraris reduct of translate(P) at M
     iff M models the normalized reduct of P at M
         and J models the normalized reduct of P at M.
   ```

6. `answer_set_iff` transports subset minimality through those correspondences:

   ```lean
   Semantics.Stable P M ↔ Ferraris.Stable M (translate P)
   ```

`frozen_subset_model_iff` specializes step 5 to a candidate already known to
satisfy its normalized reduct. The `applicable_` corollaries expose the direct
rule map needed at the ground-program boundary.
`ferraris_answer_set_iff_closure` then combines the bridge with the existing
least-closure theorem: normalized closure plus constraints is an exact test of
Ferraris answer-set membership for this fragment. The reduct remains the
acceptance foundation.

## Scope and remaining correspondence

Both sides use the same atom type. It may be finite, such as `Fin atom_count`,
and may include unused atoms. No assumption that every atom occurs in a rule is
needed; subset minimality ranges over the same universe on both sides. The
theorems do not establish the executable conversion from source predicates and
arguments to those dense atoms.

The bridge covers normalized single-head rules and constraints, including
frozen choice gates. It does not translate arbitrary source disjunctions,
aggregates, objectives or theory atoms into that fragment. General Ferraris
formulas retain their independently defined reduct semantics.

The remaining implementation obligations include source grounding and filter
discharge; atom-index correspondence; faithful Rust DAG construction and graph
validation; resource and interruption behavior; and scalar, Rayon and WGSL
execution. `from_ground_program_supported` additionally inserts candidate
support guards; this bridge does not prove that separate operation's concrete
implementation. The checked Lean program is a mathematical deliverable, not a
claim that the entire solver is formally verified.
