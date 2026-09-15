import Zetesis.Observations

/-!
# Finite inverse observation bindings

These mathematical integer laws justify deriving one candidate for a translation
or reflected translation, then retaining the complete authored guard. The finite
scalar interval remains explicit. They do not equate mathematical integers with
Rust's checked i32 arithmetic or prove multiplication inversion, source safety,
compiled alternative selection, resource limits, or error precedence.

Structural transaction and alternative coverage use the existing independent
`StructuralBindings` and `Observations` laws; anonymous negation uses the existing
`ProjectedConditionals` witness-projection laws.
-/

namespace Zetesis.ObservationBindings

/-- Translation has exactly one inverse candidate. -/
theorem translation_inverse (value offset target : Int) :
    value + offset = target ↔ value = target - offset := by
  omega

/-- Reflection followed by translation also has one inverse candidate. -/
theorem reflected_inverse (value offset target : Int) :
    offset - value = target ↔ value = offset - target := by
  omega

/-- A bounded translation match exists exactly when its sole candidate belongs
    to the original interval and satisfies the retained complete guard. No
    out-of-range candidate or independent choice on another edge is admitted. -/
theorem bounded_translation (lower upper offset target : Int) (guard : Int → Prop) :
    (∃ value, lower ≤ value ∧ value ≤ upper ∧ value + offset = target ∧ guard value) ↔
      lower ≤ target - offset ∧ target - offset ≤ upper ∧ guard (target - offset) := by
  have sound : (∃ value, lower ≤ value ∧ value ≤ upper ∧ value + offset = target ∧ guard value) →
      lower ≤ target - offset ∧ target - offset ≤ upper ∧ guard (target - offset) := by
    rintro ⟨value, lower_bound, upper_bound, equation, enabled⟩
    have candidate : value = target - offset := (translation_inverse value offset target).mp equation
    subst value
    exact ⟨lower_bound, upper_bound, enabled⟩
  have complete : (lower ≤ target - offset ∧ target - offset ≤ upper ∧ guard (target - offset)) →
      ∃ value, lower ≤ value ∧ value ≤ upper ∧ value + offset = target ∧ guard value := by
    rintro ⟨lower_bound, upper_bound, enabled⟩
    exact ⟨target - offset, lower_bound, upper_bound,
      (translation_inverse (target - offset) offset target).mpr rfl, enabled⟩
  exact ⟨sound, complete⟩

end Zetesis.ObservationBindings
