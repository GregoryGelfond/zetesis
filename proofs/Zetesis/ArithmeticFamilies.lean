/-!
# Arithmetic definedness of a substitution family

A completed source family distinguishes a produced instance, a defined but false
instance, an evaluated zero divisor and a fatal evaluation failure. An empty
positive join has no instances; it is not an arithmetic failure. A zero divisor
is tolerable only when the same family contains a jointly defined instance.
Defined but false instances provide that witness without producing a rule.

The family is scoped to one original source occurrence and fixed outer binding.
Combining different outer bindings can hide an entirely undefined local family.
Completeness is also essential: an unfinished prefix cannot establish that no
defined instance exists. Order and duplicate visits do not affect the verdict.

These laws specify admission and warning policy after complete classification.
They do not establish Rust expression classification, checked arithmetic,
evaluation order, source-family enumeration, support completion or resource
accounting. Fatal failures include overflow, nonnumeric arithmetic and invalid
exponents; they are never represented by the zero-divisor alternative.
-/

namespace Zetesis.ArithmeticFamilies

/-- The outcome of one complete substitution, after any explicit exclusions.
`filtered` means defined but false. An excluded operation is not evaluated and
therefore cannot supply `zeroDivisor`. Absence of a positive join supplies no
outcome at all. -/
inductive Outcome where
  | produced
  | filtered
  | zeroDivisor
  | fatal
  deriving DecidableEq

/-- Definedness does not require production of a ground rule. -/
def HasDefined (family : List Outcome) : Prop :=
  .produced ∈ family ∨ .filtered ∈ family

/-- A complete family has no fatal failure and no unaccompanied zero divisor. -/
def Admissible (family : List Outcome) : Prop :=
  .fatal ∉ family ∧ (.zeroDivisor ∈ family → HasDefined family)

/-- A successful family warns exactly when it omitted an undefined instance. -/
def Warned (family : List Outcome) : Prop :=
  Admissible family ∧ .zeroDivisor ∈ family

/-- An absent fact or empty join is a valid empty family. -/
theorem empty_admissible : Admissible [] := by
  simp [Admissible]

/-- An empty join cannot issue an arithmetic warning. -/
theorem empty_unwarned : ¬ Warned [] := by
  simp [Warned]

/-- A fatal arithmetic error cannot be rescued by other substitutions. -/
theorem fatal_refused (family : List Outcome) (failed : .fatal ∈ family) :
    ¬ Admissible family := by
  intro admitted
  exact admitted.1 failed

/-- A defined but false instance supplies the same admission witness as a
produced instance. The only remaining requirement is absence of fatal errors. -/
theorem filtered_witness (family : List Outcome) (defined : .filtered ∈ family)
    (safe : .fatal ∉ family) : Admissible family := by
  refine ⟨safe, ?_⟩
  intro _
  exact Or.inr defined

/-- With a zero-divisor occurrence, admission requires a jointly defined
substitution in that same completed family. -/
theorem zero_divisor_requires_defined (family : List Outcome)
    (admitted : Admissible family) (undefined : .zeroDivisor ∈ family) :
    HasDefined family := by
  exact admitted.2 undefined

/-- If every nonempty instance is undefined, the family is refused. -/
theorem entirely_undefined_refused (family : List Outcome)
    (undefined : .zeroDivisor ∈ family) (no_defined : ¬ HasDefined family) :
    ¬ Admissible family := by
  intro admitted
  exact no_defined (zero_divisor_requires_defined family admitted undefined)

/-- Reordering or revisiting substitutions preserves admission when it preserves
the classified outcomes. Completeness of that coverage is a caller obligation. -/
theorem admission_by_outcomes (before after : List Outcome)
    (same : ∀ outcome, outcome ∈ before ↔ outcome ∈ after) :
    Admissible before ↔ Admissible after := by
  simp only [Admissible, HasDefined, same]

/-- The same coverage relation preserves whether a successful family warns. -/
theorem warning_by_outcomes (before after : List Outcome)
    (same : ∀ outcome, outcome ∈ before ↔ outcome ∈ after) :
    Warned before ↔ Warned after := by
  have admission : Admissible before ↔ Admissible after :=
    admission_by_outcomes before after same
  simp only [Warned, admission, same]

/-- A bad prefix can acquire a defined witness later. Refusal therefore requires
complete family evidence, not the first support round or batch. -/
theorem prefix_does_not_establish_refusal :
    ¬ Admissible [.zeroDivisor] ∧ Admissible [.zeroDivisor, .filtered] := by
  simp [Admissible, HasDefined]

/-- Flattening local families loses their boundary: a valid outer binding can
conceal another binding's undefined family. Admission must remain pointwise. -/
theorem flattening_can_conceal_refusal :
    Admissible ([.zeroDivisor] ++ [.filtered]) ∧
      ¬ (Admissible [.zeroDivisor] ∧ Admissible [.filtered]) := by
  simp [Admissible, HasDefined]

end Zetesis.ArithmeticFamilies
