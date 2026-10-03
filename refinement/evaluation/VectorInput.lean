import InsertionLoop

open Aeneas Aeneas.Std Result

/-!
# Finite input from an owned atom vector

The standard vector iterator yields the stored atoms in order and stops at the
end. These laws derive the insertion loop's finite-input contract from the
actual imported vector iterator and conversion operations. The stored list is
not an assumed output of the constructor, and duplicate atoms remain allowed.

The imported iterator is a sequence model of Rust's standard library. Its
correspondence to Rust, including ownership and destruction, is trusted; no
allocator or custom-iterator behavior is inferred from this model.
-/
namespace VectorInput

/-- The actual owned-vector iterator satisfies the finite-input contract for
its entire stored sequence. No input ordering or distinctness premise is needed.

Proof: the empty vector returns None. A nonempty vector returns its head and the
strictly shorter tail, for which the induction hypothesis supplies the remaining
actual next calls. The vector's length bound also bounds its returned tail. -/
theorem finite (input : alloc.vec.Vec Usize) :
    InsertionLoop.FiniteInput (core.iter.traits.iterator.IteratorVecIntoIter Usize)
      input input.val := by
  have construct (values : List Usize) : ∀ input : alloc.vec.Vec Usize,
      input.val = values →
      InsertionLoop.FiniteInput (core.iter.traits.iterator.IteratorVecIntoIter Usize)
        input values := by
    induction values with
    | nil =>
        intro input empty
        apply InsertionLoop.FiniteInput.exhausted input input
        change alloc.vec.into_iter.IteratorIntoIter.next input = ok (none, input)
        unfold alloc.vec.into_iter.IteratorIntoIter.next
        split <;> simp_all
        rfl
    | cons atom tail induction =>
        intro input contents
        have bounded : tail.length ≤ Usize.max := by
          have stored := alloc.vec.Vec.property input
          rw [contents] at stored
          simp only [List.length_cons] at stored
          omega
        let next : alloc.vec.Vec Usize := .from tail bounded
        have returned :
            (core.iter.traits.iterator.IteratorVecIntoIter Usize).next input =
              ok (some atom, next) := by
          change alloc.vec.into_iter.IteratorIntoIter.next input = ok (some atom, next)
          unfold alloc.vec.into_iter.IteratorIntoIter.next
          split <;> simp_all [next]
          rfl
        exact .yielded input next atom tail returned (induction next (by simp [next]))
  exact construct input.val input rfl

/-- Converting an owned atom vector uses the actual IntoIterator operation and
produces a finite iterator over exactly its stored sequence. This supplies the
ordinary solver input without an external iterator-correctness premise. -/
theorem converted (atoms : alloc.vec.Vec Usize) :
    ∃ iterator,
      (core.iter.traits.collect.IntoIteratorVec Usize).into_iter atoms = ok iterator ∧
      InsertionLoop.FiniteInput (core.iter.traits.iterator.IteratorVecIntoIter Usize)
        iterator atoms.val := by
  exact ⟨atoms, rfl, finite atoms⟩

end VectorInput
