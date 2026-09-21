import Zetesis.ConstrainedPositive

/-!
# Finite constraint streams over an unchanged producer theory

An occurrence identifies one admitted ground constraint body. A finite source
list retains those occurrences; a partition must cover it by permutation. A
Boolean test reads one fixed candidate. The bounded scanner either names a
violating occurrence, completes, or retains an unchecked suffix. Its invariant
accounts for a checked prefix without treating interruption as satisfaction.

Completed partitions have the same satisfaction verdict as the source list,
regardless of order or chunk boundaries. Pointwise agreement between the test
and each body's original truth connects that executable scan to Ferraris
constraints. The existing constraint-append law then applies to any retained
producer theory, not only a positive one. First violation order is not preserved.

The finite source denotation and pointwise evaluator agreement are premises.
Source lowering, local-family and aggregate witness completeness, arithmetic
admission, concrete cursors, owner identities, cancellation and checked resource
accounting remain Rust obligations. The list here is a mathematical denotation,
not a requirement to materialize every constraint in the implementation.
-/

namespace Zetesis.StreamedConstraints

open Ferraris

universe u v
variable {ι : Type u} {A : Type v}

/-- Every occurrence in this finite list has a false body in the candidate. -/
def Clear (test : ι → Bool) (source : List ι) : Prop :=
  ∀ occurrence, occurrence ∈ source → test occurrence ≠ true

/-- A violation carries its source occurrence; pending retains all unread work. -/
inductive Outcome (ι : Type u) where
  | violated : ι → Outcome ι
  | complete : Outcome ι
  | pending : List ι → Outcome ι

/-- One fuel unit inspects one occurrence or observes the empty suffix.
Zero fuel always yields, including when the suffix is already empty. -/
def scan (test : ι → Bool) : Nat → List ι → Outcome ι
  | 0, source => .pending source
  | _ + 1, [] => .complete
  | fuel + 1, occurrence :: rest =>
      if test occurrence = true then .violated occurrence
      else scan test fuel rest

/-- A result either authenticates a violation, certifies the whole list, or
retains precisely the suffix after a prefix whose bodies were false. -/
def Preserves (test : ι → Bool) (source : List ι) : Outcome ι → Prop
  | .violated occurrence => occurrence ∈ source ∧ test occurrence = true
  | .complete => Clear test source
  | .pending rest => ∃ checked, source = checked ++ rest ∧ Clear test checked

/-- A concatenation is clear exactly when both parts are clear. This is the
composition rule for completed chunks, including empty chunks. -/
theorem clear_append (test : ι → Bool) (first second : List ι) :
    Clear test (first ++ second) ↔ Clear test first ∧ Clear test second := by
  simp only [Clear, List.mem_append, or_imp, forall_and]

/-- Reordering occurrences preserves satisfaction. The permutation premise
also retains occurrence multiplicity, though repeated constraints do not change truth. -/
theorem clear_permutation (test : ι → Bool) {first second : List ι}
    (same : first.Perm second) : Clear test first ↔ Clear test second := by
  constructor
  · intro clear occurrence member
    exact clear occurrence (same.mem_iff.mpr member)
  · intro clear occurrence member
    exact clear occurrence (same.mem_iff.mp member)

/-- Every bounded scan preserves its source accounting.

Proof outline: zero fuel leaves the whole list pending. An empty list completes.
A true body authenticates its own violation. Otherwise the head is false, so
the tail's result extends either its certificate or its checked prefix by that head. -/
theorem scan_preserves (test : ι → Bool) (fuel : Nat) (source : List ι) :
    Preserves test source (scan test fuel source) := by
  induction fuel generalizing source with
  | zero =>
    refine ⟨[], rfl, ?_⟩
    simp [Clear]
  | succ fuel ih =>
    cases source with
    | nil => simp [scan, Preserves, Clear]
    | cons occurrence rest =>
      by_cases fired : test occurrence = true
      · simp [scan, fired, Preserves]
      · have remaining : Preserves test rest (scan test fuel rest) := ih rest
        simp only [scan, if_neg fired]
        cases result : scan test fuel rest with
        | violated witness =>
          rw [result] at remaining
          exact ⟨List.mem_cons_of_mem occurrence remaining.1, remaining.2⟩
        | complete =>
          rw [result] at remaining
          intro value member
          rcases List.mem_cons.mp member with same | inside
          · subst value
            exact fired
          · exact remaining value inside
        | pending suffix =>
          rw [result] at remaining
          obtain ⟨checked, partition, clear⟩ := remaining
          refine ⟨occurrence :: checked, ?_, ?_⟩
          · simp only [List.cons_append, partition]
          · intro value member
            rcases List.mem_cons.mp member with same | inside
            · subst value
              exact fired
            · exact clear value inside

/-- More fuel than the number of occurrences permits either a violation or
completion. This finite bound is not a bound on Rust join or expression work. -/
theorem enough_fuel_no_pending (test : ι → Bool) (fuel : Nat)
    (source rest : List ι) (enough : source.length < fuel) :
    scan test fuel source ≠ .pending rest := by
  induction fuel generalizing source with
  | zero => omega
  | succ fuel ih =>
    cases source with
    | nil => simp [scan]
    | cons occurrence tail =>
      by_cases fired : test occurrence = true
      · simp [scan, fired]
      · simp only [scan, if_neg fired]
        apply ih tail
        simp only [List.length_cons] at enough
        omega

/-- A sufficiently fuelled scan completes exactly when every body is false.
Soundness uses the scan invariant. For completeness, a violation contradicts
the clear-list premise, and the length bound excludes a pending result. -/
theorem scan_complete_iff (test : ι → Bool) (fuel : Nat) (source : List ι)
    (enough : source.length < fuel) :
    scan test fuel source = .complete ↔ Clear test source := by
  constructor
  · intro complete
    have retained : Preserves test source (scan test fuel source) :=
      scan_preserves test fuel source
    simpa only [complete, Preserves] using retained
  · intro clear
    cases result : scan test fuel source with
    | complete => rfl
    | violated occurrence =>
      have witness : occurrence ∈ source ∧ test occurrence = true := by
        simpa only [result, Preserves] using scan_preserves test fuel source
      exact False.elim (clear occurrence witness.1 witness.2)
    | pending rest =>
      exact False.elim (enough_fuel_no_pending test fuel source rest enough result)

/-- A sufficiently fuelled scan finds a violation exactly when a source body
fires. The identity of the first witness can depend on traversal order. -/
theorem scan_violation_iff (test : ι → Bool) (fuel : Nat) (source : List ι)
    (enough : source.length < fuel) :
    (∃ occurrence, scan test fuel source = .violated occurrence) ↔
      ∃ occurrence ∈ source, test occurrence = true := by
  constructor
  · rintro ⟨occurrence, result⟩
    have witness : occurrence ∈ source ∧ test occurrence = true := by
      simpa only [result, Preserves] using scan_preserves test fuel source
    exact ⟨occurrence, witness.1, witness.2⟩
  · rintro ⟨occurrence, member, fired⟩
    cases result : scan test fuel source with
    | violated witness => exact ⟨witness, rfl⟩
    | complete =>
      have clear : Clear test source :=
        (scan_complete_iff test fuel source enough).mp result
      exact False.elim (clear occurrence member fired)
    | pending rest =>
      exact False.elim (enough_fuel_no_pending test fuel source rest enough result)

/-- Every completed part is clear exactly when the covered original family is
clear. Flattening accounts for chunk boundaries; permutation accounts for order.
All parts must complete: an unfinished worker cannot supply the left premise. -/
theorem completed_partition (test : ι → Bool) (source : List ι)
    (parts : List (List ι)) (coverage : parts.flatten.Perm source) :
    (∀ part ∈ parts, scan test (part.length + 1) part = .complete) ↔
      Clear test source := by
  constructor
  · intro completed
    apply (clear_permutation test coverage).mp
    intro occurrence member
    obtain ⟨part, present, inside⟩ := List.mem_flatten.mp member
    have clear : Clear test part :=
      (scan_complete_iff test (part.length + 1) part (by omega)).mp (completed part present)
    exact clear occurrence inside
  · intro clear part present
    apply (scan_complete_iff test (part.length + 1) part (by omega)).mpr
    have flattened : Clear test parts.flatten := (clear_permutation test coverage).mpr clear
    intro occurrence inside
    exact flattened occurrence (List.mem_flatten.mpr ⟨part, present, inside⟩)

/-- Any two complete partitions of the same source give the same completed
satisfaction verdict. They need not share chunk sizes or occurrence order. -/
theorem partition_invariance (test : ι → Bool) (source : List ι)
    (first second : List (List ι))
    (firstCoverage : first.flatten.Perm source) (secondCoverage : second.flatten.Perm source) :
    (∀ part ∈ first, scan test (part.length + 1) part = .complete) ↔
      (∀ part ∈ second, scan test (part.length + 1) part = .complete) :=
  (completed_partition test source first firstCoverage).trans
    (completed_partition test source second secondCoverage).symm

/-- Pointwise original-truth agreement connects the scan to the admitted
constraint formulas. Source coverage is explicit in the finite occurrence list;
this statement does not infer it from the evaluator or from finite storage. -/
theorem clear_iff_models (test : ι → Bool) (source : List ι)
    (body : ι → Formula A) (M : Atoms A)
    (evaluation : ∀ occurrence ∈ source,
      test occurrence = true ↔ Satisfies M (body occurrence)) :
    Clear test source ↔ Models M ((source.map body).map Ferraris.Neg) := by
  constructor
  · intro clear formula member
    obtain ⟨original, present, rfl⟩ := List.mem_map.mp member
    obtain ⟨occurrence, inside, rfl⟩ := List.mem_map.mp present
    intro satisfied
    exact clear occurrence inside ((evaluation occurrence inside).mpr satisfied)
  · intro models occurrence inside fired
    have present : Ferraris.Neg (body occurrence) ∈ (source.map body).map Ferraris.Neg :=
      List.mem_map.mpr ⟨body occurrence, List.mem_map.mpr ⟨occurrence, inside, rfl⟩, rfl⟩
    exact models _ present ((evaluation occurrence inside).mp fired)

/-- Completed streamed checks filter the answer sets of any retained theory.

Proof outline: the existing append law separates producer stability from original
constraint satisfaction. Pointwise evaluation turns that satisfaction into a
clear source list; complete partition coverage turns it into completed scans.
Neither positivity of the retained theory nor monotonicity of the bodies is needed. -/
theorem stable_iff_completed_partition (P : Theory A) (source : List ι)
    (body : ι → Formula A) (M : Atoms A) (test : ι → Bool)
    (evaluation : ∀ occurrence ∈ source,
      test occurrence = true ↔ Satisfies M (body occurrence))
    (parts : List (List ι)) (coverage : parts.flatten.Perm source) :
    Stable M (P ++ (source.map body).map Ferraris.Neg) ↔
      Stable M P ∧ ∀ part ∈ parts, scan test (part.length + 1) part = .complete := by
  have completedModels :
      (∀ part ∈ parts, scan test (part.length + 1) part = .complete) ↔
        Models M ((source.map body).map Ferraris.Neg) :=
    (completed_partition test source parts coverage).trans
      (clear_iff_models test source body M evaluation)
  rw [ConstrainedPositive.stable_append_constraints]
  exact and_congr_right fun _ => completedModels.symm

/-- A checked false prefix can leave a true body pending. Therefore a yielded
scan, even after useful work, does not establish satisfaction of the family. -/
theorem pending_can_hide_violation :
    scan (fun value : Bool => value) 1 [false, true] = .pending [true] ∧
      ¬ Clear (fun value : Bool => value) [false, true] := by
  simp [scan, Clear]

end Zetesis.StreamedConstraints
