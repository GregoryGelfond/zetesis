/-!
# Content-order interning

The interner gives each generation's new content dense identities in the order
of its one comparison. `IdOrderEqCompare` names the resulting law: within one
generation, identity order equals content order. The Rust interner
(`zetesis_core::intern`) relies on it to decide same-generation comparisons by
identity alone. Here the law is a named hypothesis, not an axiom: the theorems
below state what follows from it, and `freeze_establishes_law` proves that the
freeze construction, numbering a strictly sorted list by position, satisfies it.

These laws concern order and identity. They do not verify the Rust hash
tables, shard locking, the sort implementation or its work accounting.
-/

namespace Zetesis.ContentOrderInterning

universe u
variable {α : Type u}

/-- Within one generation, identity order equals the comparison's strict order. -/
def IdOrderEqCompare (generation : List α) (id : α → Nat) (lt : α → α → Prop) : Prop :=
  ∀ a, a ∈ generation → ∀ b, b ∈ generation → (id a < id b ↔ lt a b)

/-- Under the law, identities of one generation are distinct for distinct
content: equal identities are related neither way, so a trichotomous order
makes the contents equal. -/
theorem id_injective_of_law {generation : List α} {id : α → Nat} {lt : α → α → Prop}
    (law : IdOrderEqCompare generation id lt)
    (trichotomous : ∀ a b, lt a b ∨ a = b ∨ lt b a)
    {a b : α} (ha : a ∈ generation) (hb : b ∈ generation) (same : id a = id b) : a = b := by
  have notAB : ¬ lt a b := by
    intro h
    have := (law a ha b hb).mpr h
    omega
  have notBA : ¬ lt b a := by
    intro h
    have := (law b hb a ha).mpr h
    omega
  rcases trichotomous a b with h | h | h
  · exact absurd h notAB
  · exact h
  · exact absurd h notBA

/-- Under the law, sorting a generation by identity is sorting it by content:
a list whose identities increase is strictly sorted by the comparison. -/
theorem sorted_by_id_is_sorted_by_compare {generation : List α} {id : α → Nat}
    {lt : α → α → Prop} (law : IdOrderEqCompare generation id lt) :
    ∀ l : List α, (∀ x, x ∈ l → x ∈ generation) →
      l.Pairwise (fun x y => id x < id y) → l.Pairwise lt := by
  intro l inGeneration sorted
  induction l with
  | nil => exact List.Pairwise.nil
  | cons x rest ih =>
    rcases List.pairwise_cons.mp sorted with ⟨head, tail⟩
    have restIn : ∀ y, y ∈ rest → y ∈ generation :=
      fun y hy => inGeneration y (List.mem_cons_of_mem x hy)
    have xIn : x ∈ generation := inGeneration x List.mem_cons_self
    refine List.pairwise_cons.mpr ⟨?_, ih restIn tail⟩
    intro y hy
    exact (law x xIn y (restIn y hy)).mp (head y hy)

/-- The freeze construction: a generation listed strictly sorted by the
comparison, each element numbered by its position, satisfies the law. -/
theorem freeze_establishes_law [DecidableEq α] {lt : α → α → Prop}
    (irreflexive : ∀ a, ¬ lt a a) (transitive : ∀ a b c, lt a b → lt b c → lt a c)
    (sorted : List α) (strict : sorted.Pairwise lt) :
    IdOrderEqCompare sorted (fun a => sorted.idxOf a) lt := by
  -- Positions below the length name the element at that position.
  have before : ∀ a, a ∈ sorted → ∀ b, b ∈ sorted →
      sorted.idxOf a < sorted.idxOf b → lt a b := by
    intro a ha b hb lessIdx
    have ia : sorted.idxOf a < sorted.length := List.idxOf_lt_length_iff.mpr ha
    have ib : sorted.idxOf b < sorted.length := List.idxOf_lt_length_iff.mpr hb
    have ga : sorted[sorted.idxOf a] = a := List.getElem_idxOf ia
    have gb : sorted[sorted.idxOf b] = b := List.getElem_idxOf ib
    have rel := List.pairwise_iff_getElem.mp strict _ _ ia ib lessIdx
    rw [ga, gb] at rel
    exact rel
  intro a ha b hb
  constructor
  · exact before a ha b hb
  · intro ab
    rcases Nat.lt_trichotomy (sorted.idxOf a) (sorted.idxOf b) with h | h | h
    · exact h
    · have ia : sorted.idxOf a < sorted.length := List.idxOf_lt_length_iff.mpr ha
      have ib : sorted.idxOf b < sorted.length := List.idxOf_lt_length_iff.mpr hb
      have ga : sorted[sorted.idxOf a] = a := List.getElem_idxOf ia
      have gb : sorted[sorted.idxOf b] = b := List.getElem_idxOf ib
      have equal : a = b := by
        have : sorted[sorted.idxOf a] = sorted[sorted.idxOf b] := by simp [h]
        rw [ga, gb] at this
        exact this
      subst equal
      exact absurd ab (irreflexive a)
    · exact absurd (transitive a b a ab (before b hb a ha h)) (irreflexive a)

end Zetesis.ContentOrderInterning
