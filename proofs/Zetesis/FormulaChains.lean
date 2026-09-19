import Zetesis.FormulaBounds

/-!
# Chains of one connective read as one node

A clause of `k` literals is admitted as a chain of `k − 1` binary
disjunctions, and a body of `k` literals as a chain of conjunctions. The
narrowing reads such a chain as one node with `k` operands: a disjunction is
sure when one operand is, impossible when all are, and a disjunction known to
hold with all operands but one known to fail forces that one; a conjunction
dually. Each of these rules is admissible in `FormulaBounds.Known`: it is a
sequence of the binary rules along the chain, one per internal node, which is
what the proofs below unfold by induction on the chain. The Rust narrowing
applies the chain rules with two counters per chain, the operands known to
hold and the operands known to fail, so a decision costs one step per
occurrence of its atom rather than a walk to the root of every chain.

A chain is any tree of one connective whose leaves are arbitrary formulas; the
leaves are listed left to right, one per operand position, so a formula
occurring at two positions is two leaves. Nothing restricts what a leaf is: a
shared subformula with parents outside the chain is a leaf of it.

The declarations live in the `FormulaBounds` namespace, since they extend its
`Known` with derived rules; the module rests on `FormulaBounds` for the
knowledge and its binary rules. That the Rust chains are such trees, maximal
trees of one connective whose inner nodes have that one parent, and that the
two counters count the operands known to hold and to fail, are the Rust
obligations.
-/

namespace Zetesis.FormulaBounds

open Ferraris

universe u

variable {α : Type u}

/-- A formula that is a tree of disjunctions over the listed leaves. -/
inductive DisjChain : Formula α → List (Formula α) → Prop
  | leaf (F : Formula α) : DisjChain F [F]
  | node {F G : Formula α} {ls rs : List (Formula α)}
      (left : DisjChain F ls) (right : DisjChain G rs) : DisjChain (.disj F G) (ls ++ rs)

/-- A formula that is a tree of conjunctions over the listed leaves. -/
inductive ConjChain : Formula α → List (Formula α) → Prop
  | leaf (F : Formula α) : ConjChain F [F]
  | node {F G : Formula α} {ls rs : List (Formula α)}
      (left : ConjChain F ls) (right : ConjChain G rs) : ConjChain (.conj F G) (ls ++ rs)

/-- A position in the concatenation of two lists lies in one of them: the
leaves before and after it split accordingly. -/
theorem position_in_append {X : Type u} {ls rs before after : List X} {u : X}
    (h : ls ++ rs = before ++ u :: after) :
    (∃ b a, ls = b ++ u :: a ∧ before = b ∧ after = a ++ rs) ∨
      (∃ b, rs = b ++ u :: after ∧ before = ls ++ b) := by
  induction ls generalizing before with
  | nil =>
    exact Or.inr ⟨before, h, rfl⟩
  | cons x xs ih =>
    cases before with
    | nil =>
      have head : x = u := (List.cons.inj h).1
      have tail : xs ++ rs = after := (List.cons.inj h).2
      exact Or.inl ⟨[], xs, by subst head; rfl, rfl, tail.symm⟩
    | cons y ys =>
      have head : x = y := (List.cons.inj h).1
      have tail : xs ++ rs = ys ++ u :: after := (List.cons.inj h).2
      rcases ih tail with ⟨b, a, split, hb, ha⟩ | ⟨b, split, hb⟩
      · exact Or.inl ⟨y :: b, a, by rw [head, split]; rfl, by rw [hb], ha⟩
      · exact Or.inr ⟨b, split, by rw [head, hb, List.cons_append]⟩

variable (T : Theory α) (c : Cube α)

/-- A disjunction chain with a leaf known to hold is known to hold:
`disj_up_left` or `disj_up_right` at each node above the leaf. -/
theorem disj_chain_sure {F : Formula α} {ls : List (Formula α)} (chain : DisjChain F ls)
    {L : Formula α} (member : L ∈ ls) (h : Known T c L true) : Known T c F true := by
  induction chain with
  | leaf F =>
    have eq : L = F := List.mem_singleton.mp member
    exact eq ▸ h
  | node _ _ ihLeft ihRight =>
    rcases List.mem_append.mp member with inLeft | inRight
    · exact .disj_up_left (ihLeft inLeft)
    · exact .disj_up_right (ihRight inRight)

/-- A disjunction chain whose every leaf is known to fail is known to fail:
`disj_up` at each node. -/
theorem disj_chain_never {F : Formula α} {ls : List (Formula α)} (chain : DisjChain F ls)
    (h : ∀ L ∈ ls, Known T c L false) : Known T c F false := by
  induction chain with
  | leaf F => exact h F (List.mem_singleton.mpr rfl)
  | node _ _ ihLeft ihRight =>
    exact .disj_up (ihLeft fun L hL => h L (List.mem_append_left _ hL))
      (ihRight fun L hL => h L (List.mem_append_right _ hL))

/-- The unit rule of a disjunction chain: known to hold, with every leaf but
the one at a given position known to fail, it forces that leaf. Down the
chain, the subtree holding the position is known to hold by `disj_left` or
`disj_right`, since the other subtree fails by `disj_chain_never`. -/
theorem disj_chain_unit {F : Formula α} {ls : List (Formula α)} (chain : DisjChain F ls)
    (sure : Known T c F true) {before after : List (Formula α)} {U : Formula α}
    (position : ls = before ++ U :: after)
    (failedBefore : ∀ L ∈ before, Known T c L false)
    (failedAfter : ∀ L ∈ after, Known T c L false) : Known T c U true := by
  induction chain generalizing before after with
  | leaf F =>
    -- The one leaf is the position: nothing lies before or after it.
    cases before with
    | nil =>
      simp only [List.nil_append, List.cons.injEq] at position
      exact position.1 ▸ sure
    | cons _ tail =>
      have lengths := congrArg List.length position
      simp at lengths
  | node left right ihLeft ihRight =>
    rcases position_in_append position with ⟨b, a, split, hb, ha⟩ | ⟨b, split, hb⟩
    · -- The position lies in the left subtree; the right subtree's leaves
      -- all lie after it and fail, so the right subtree fails and the left
      -- one holds.
      have rightFails : Known T c _ false :=
        disj_chain_never T c right fun L hL =>
          failedAfter L (ha ▸ List.mem_append_right _ hL)
      have leftHolds : Known T c _ true := .disj_left sure rightFails
      exact ihLeft leftHolds split (fun L hL => failedBefore L (hb ▸ hL))
        (fun L hL => failedAfter L (ha ▸ List.mem_append_left _ hL))
    · -- The position lies in the right subtree; the left subtree's leaves
      -- all lie before it and fail.
      have leftFails : Known T c _ false :=
        disj_chain_never T c left fun L hL =>
          failedBefore L (hb ▸ List.mem_append_left _ hL)
      have rightHolds : Known T c _ true := .disj_right sure leftFails
      exact ihRight rightHolds split (fun L hL => failedBefore L (hb ▸ List.mem_append_right _ hL))
        failedAfter

/-- A conjunction chain whose every leaf is known to hold is known to hold:
`conj_up` at each node. -/
theorem conj_chain_sure {F : Formula α} {ls : List (Formula α)} (chain : ConjChain F ls)
    (h : ∀ L ∈ ls, Known T c L true) : Known T c F true := by
  induction chain with
  | leaf F => exact h F (List.mem_singleton.mpr rfl)
  | node _ _ ihLeft ihRight =>
    exact .conj_up (ihLeft fun L hL => h L (List.mem_append_left _ hL))
      (ihRight fun L hL => h L (List.mem_append_right _ hL))

/-- A conjunction chain with a leaf known to fail is known to fail:
`conj_up_left` or `conj_up_right` at each node above the leaf. -/
theorem conj_chain_never {F : Formula α} {ls : List (Formula α)} (chain : ConjChain F ls)
    {L : Formula α} (member : L ∈ ls) (h : Known T c L false) : Known T c F false := by
  induction chain with
  | leaf F =>
    have eq : L = F := List.mem_singleton.mp member
    exact eq ▸ h
  | node _ _ ihLeft ihRight =>
    rcases List.mem_append.mp member with inLeft | inRight
    · exact .conj_up_left (ihLeft inLeft)
    · exact .conj_up_right (ihRight inRight)

/-- The unit rule of a conjunction chain: known to fail, with every leaf but
the one at a given position known to hold, it cuts that leaf; the dual of
`disj_chain_unit` through `not_conj_left` and `not_conj_right`. -/
theorem conj_chain_unit {F : Formula α} {ls : List (Formula α)} (chain : ConjChain F ls)
    (never : Known T c F false) {before after : List (Formula α)} {U : Formula α}
    (position : ls = before ++ U :: after)
    (heldBefore : ∀ L ∈ before, Known T c L true)
    (heldAfter : ∀ L ∈ after, Known T c L true) : Known T c U false := by
  induction chain generalizing before after with
  | leaf F =>
    cases before with
    | nil =>
      simp only [List.nil_append, List.cons.injEq] at position
      exact position.1 ▸ never
    | cons _ tail =>
      have lengths := congrArg List.length position
      simp at lengths
  | node left right ihLeft ihRight =>
    rcases position_in_append position with ⟨b, a, split, hb, ha⟩ | ⟨b, split, hb⟩
    · have rightHolds : Known T c _ true :=
        conj_chain_sure T c right fun L hL =>
          heldAfter L (ha ▸ List.mem_append_right _ hL)
      have leftFails : Known T c _ false := .not_conj_left never rightHolds
      exact ihLeft leftFails split (fun L hL => heldBefore L (hb ▸ hL))
        (fun L hL => heldAfter L (ha ▸ List.mem_append_left _ hL))
    · have leftHolds : Known T c _ true :=
        conj_chain_sure T c left fun L hL =>
          heldBefore L (hb ▸ List.mem_append_left _ hL)
      have rightFails : Known T c _ false := .not_conj_right never leftHolds
      exact ihRight rightFails split (fun L hL => heldBefore L (hb ▸ List.mem_append_right _ hL))
        heldAfter

end Zetesis.FormulaBounds
