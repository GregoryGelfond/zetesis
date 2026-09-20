import Zetesis.ConstrainedPositive

/-!
# Constraints over a keyed value, asked as the one atom the key admits

A keyed relation holds, in every answer set, exactly one value for each key
its body admits and none for any other key. A constraint that reads that
value only to compare it can then ask for the one atom the comparison
demands instead of reading every value. Two patterns are asked this way, and
each has its law here.

*One value.* `:- G, p(k, Y), Y != t.` is asked as `:- G, b(k), not p(k, t).`,
where `b` is the key's body. `one_value` says the two bodies hold together in
any interpretation where the key's value is a function total over its body.

*A digit and a carry.* `:- G, p(k, Y), q(j, C), s != Y + c*C.` with a digit
`Y` in `0..c-1`, a natural carry `C` and `c ≥ 2` is asked as the two
constraints demanding `p(k, s \ c)` and `q(j, s / c)`. `digit_carry` says the
equation has that one solution, over the truncating division and remainder
the source language evaluates, and none when `s` is negative.
`digit_carry_asked` draws the consequence for two keyed values.

`asked_constraints_preserve` composes either law with
`ConstrainedPositive.stable_append_constraints`: constraints that fire
together on every answer set of the rest of the program may replace one
another without changing the program's answer sets.

That a relation is keyed, which the key analysis decides from the one choice
rule producing it, is a premise here and not a law; so are the recognition of
the two patterns, the ranges of the digit and the carry, and the evaluation
of `s` where the written constraint evaluated it. They are Rust obligations.
-/

namespace Zetesis.KeyedConstraints

open Ferraris

universe u v
variable {α : Type u} {V : Type v}

/-- In the interpretation, the atoms `p v` of one key hold a value that is a
function of the key, total over the key's body: some value when the body
holds, at most one value, and no value when the body does not hold. -/
structure Keyed (M : Atoms α) (body : Prop) (p : V → α) : Prop where
  total : body → ∃ v, M (p v)
  unique : ∀ v w, M (p v) → M (p w) → v = w
  guarded : ∀ v, M (p v) → body

/-- The key holds a value other than the demanded one exactly when its body
holds and the demanded atom is absent.

If a value other than `t` is held, the body holds because a value is held,
and `t` is not held because the value is unique. Conversely the body gives
some held value, which is not `t` because `t` is not held. -/
theorem one_value {M : Atoms α} {body : Prop} {p : V → α} (keyed : Keyed M body p) (t : V) :
    (∃ y, M (p y) ∧ y ≠ t) ↔ (body ∧ ¬ M (p t)) := by
  constructor
  · rintro ⟨y, held, differs⟩
    have bodyHolds : body := keyed.guarded y held
    have demandedAbsent : ¬ M (p t) := fun demanded =>
      differs (keyed.unique y t held demanded)
    exact ⟨bodyHolds, demandedAbsent⟩
  · rintro ⟨bodyHolds, demandedAbsent⟩
    obtain ⟨v, held⟩ := keyed.total bodyHolds
    have differs : v ≠ t := fun same => demandedAbsent (same ▸ held)
    exact ⟨v, held, differs⟩

/-- A digit `y` in `0..c-1` and a natural carry `C` satisfy `s = y + c*C`
exactly when they are the truncating remainder and quotient of `s` by `c`.

From the equation `s` is not negative, so its truncating quotient and
remainder are the Euclidean ones, which the equation and the digit's range
determine. Conversely the quotient and remainder of any `s` recompose it. No
sign of `s` is assumed: for a negative `s` both sides fail, the left because
`y + c*C` is not negative and the right because a remainder and quotient that
are both not negative recompose a number that is not negative. -/
theorem digit_carry (c s y C : Int) (base : 2 ≤ c) (digit : 0 ≤ y ∧ y < c) (carry : 0 ≤ C) :
    s = y + c * C ↔ (y = Int.tmod s c ∧ C = Int.tdiv s c) := by
  have positive : 0 < c := by omega
  constructor
  · intro equation
    have product : 0 ≤ c * C := Int.mul_nonneg (by omega) carry
    have nonneg : 0 ≤ s := by omega
    have euclid : s / c = C ∧ s % c = y :=
      (Int.ediv_emod_unique positive).mpr ⟨equation.symm, digit.1, digit.2⟩
    rw [Int.tmod_eq_emod_of_nonneg nonneg, Int.tdiv_eq_ediv_of_nonneg nonneg]
    exact ⟨euclid.2.symm, euclid.1.symm⟩
  · rintro ⟨remainder, quotient⟩
    have recomposed : c * Int.tdiv s c + Int.tmod s c = s := Int.mul_tdiv_add_tmod s c
    rw [remainder, quotient]
    omega

/-- Two keyed values, a digit and a carry, fail the equation exactly when
both bodies hold and one of the two demanded atoms is absent: the digit
`s \\ c` or the carry `s / c`.

The held digit and carry are unique, so the written constraint fires on them
alone, and by `digit_carry` they satisfy the equation exactly when they are
the demanded pair. -/
theorem digit_carry_asked {M : Atoms α} {digitBody carryBody : Prop}
    {p q : Int → α} (digits : Keyed M digitBody p) (carries : Keyed M carryBody q)
    (c s : Int) (base : 2 ≤ c)
    (digitRange : ∀ y, M (p y) → 0 ≤ y ∧ y < c) (carryRange : ∀ C, M (q C) → 0 ≤ C) :
    (∃ y C, M (p y) ∧ M (q C) ∧ s ≠ y + c * C) ↔
      (digitBody ∧ carryBody ∧ (¬ M (p (Int.tmod s c)) ∨ ¬ M (q (Int.tdiv s c)))) := by
  constructor
  · rintro ⟨y, C, heldDigit, heldCarry, fails⟩
    refine ⟨digits.guarded y heldDigit, carries.guarded C heldCarry, ?_⟩
    apply Classical.byContradiction
    intro bothDemanded
    have demandedDigit : M (p (Int.tmod s c)) :=
      Classical.byContradiction fun absent => bothDemanded (Or.inl absent)
    have demandedCarry : M (q (Int.tdiv s c)) :=
      Classical.byContradiction fun absent => bothDemanded (Or.inr absent)
    have sameDigit : y = Int.tmod s c := digits.unique _ _ heldDigit demandedDigit
    have sameCarry : C = Int.tdiv s c := carries.unique _ _ heldCarry demandedCarry
    exact fails ((digit_carry c s y C base (digitRange y heldDigit)
      (carryRange C heldCarry)).mpr ⟨sameDigit, sameCarry⟩)
  · rintro ⟨digitHolds, carryHolds, absent⟩
    obtain ⟨y, heldDigit⟩ := digits.total digitHolds
    obtain ⟨C, heldCarry⟩ := carries.total carryHolds
    refine ⟨y, C, heldDigit, heldCarry, ?_⟩
    intro equation
    have demanded : y = Int.tmod s c ∧ C = Int.tdiv s c :=
      (digit_carry c s y C base (digitRange y heldDigit) (carryRange C heldCarry)).mp equation
    rcases absent with noDigit | noCarry
    · exact noDigit (demanded.1 ▸ heldDigit)
    · exact noCarry (demanded.2 ▸ heldCarry)

/-- Constraints that fire together on every answer set of the rest of the
program may replace one another: the program with the written constraints
and the program with the asked ones have the same answer sets.

By `stable_append_constraints` each program's answer sets are the answer sets
of the rest that fire none of its constraints, and on those the two families
fire together. The premise is asked only of answer sets of the rest, which is
where a keyed relation's property holds. -/
theorem asked_constraints_preserve (P : Theory α) (written asked : List (Formula α))
    (together : ∀ M, Stable M P →
      ((∃ W ∈ written, Satisfies M W) ↔ (∃ A ∈ asked, Satisfies M A)))
    (M : Atoms α) :
    Stable M (P ++ written.map Ferraris.Neg) ↔ Stable M (P ++ asked.map Ferraris.Neg) := by
  have noneFires : ∀ bodies : List (Formula α),
      Models M (bodies.map Ferraris.Neg) ↔ ¬ ∃ B ∈ bodies, Satisfies M B := by
    intro bodies
    constructor
    · rintro models ⟨B, member, fires⟩
      exact models (Ferraris.Neg B) (List.mem_map.mpr ⟨B, member, rfl⟩) fires
    · intro none F member
      obtain ⟨B, inBodies, rfl⟩ := List.mem_map.mp member
      exact fun fires => none ⟨B, inBodies, fires⟩
  rw [ConstrainedPositive.stable_append_constraints, ConstrainedPositive.stable_append_constraints,
    noneFires written, noneFires asked]
  constructor
  · rintro ⟨stable, none⟩
    exact ⟨stable, fun fires => none ((together M stable).mpr fires)⟩
  · rintro ⟨stable, none⟩
    exact ⟨stable, fun fires => none ((together M stable).mp fires)⟩

end Zetesis.KeyedConstraints
