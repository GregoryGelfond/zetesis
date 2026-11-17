/-!
# Arithmetic definedness of a substitution family

A completed source family distinguishes a produced instance, a defined but false
instance, an evaluated zero divisor and a fatal evaluation failure. An empty
positive join has no instances; it is not an arithmetic failure. An instance
that divides by zero is omitted, whatever else its family holds: the family is
admitted without it and reports the omission as a warning. A fatal failure
refuses the family.

So admission does not depend on where a family's boundary is drawn. A family
scoped to one original source occurrence and fixed outer binding is admitted
exactly when the union of all such families is, and a family that is entirely
undefined is admitted with no instance. The warning is scoped to the
occurrence: it is raised when any of its families omitted an instance.

Completeness still matters, in one direction only. A fatal failure refuses a
family whatever follows it, and a zero divisor warns of whatever precedes or
follows it, so either verdict can be read from a prefix. That a family is
admitted, or admitted without a warning, needs every instance classified.
Order and duplicate visits do not affect the verdict.

A conditional literal keeps one instance for each binding of its condition, so
an instance whose consequent divides by zero cannot be left out: the condition
still holds or fails there. The consequent is read by what remains of it. In a
body it is the disjunction of the values its alternatives take, so with none
defined it is false, and the instance requires the condition to be false. In a
head the disjunct needs every value its head takes, so with none defined it
holds wherever the condition does. A condition that divides by zero has no
binding, and so no instance.

The links of a comparison chain are separate comparisons. A link that is
defined and false decides the chain even beside a link that divides by zero;
a chain is undefined only when no link is false and one divides by zero.

These laws specify admission and warning policy after complete classification,
the value an undefined consequent or head has, and the value of a chain from
its links. They do not establish Rust expression classification, checked
arithmetic, evaluation order, source-family enumeration, formula emission,
support completion or resource accounting. Fatal failures include overflow,
nonnumeric arithmetic and invalid exponents; they are never represented by the
zero-divisor alternative, and a fatal link refuses whatever `chain` returns.
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

/-- A complete family is admitted when no substitution failed fatally. -/
def Admissible (family : List Outcome) : Prop :=
  .fatal ∉ family

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
  exact admitted failed

/-- A zero divisor refuses nothing: a family without a fatal failure is
admitted whether or not any of its substitutions is defined. -/
theorem zero_divisor_admitted (family : List Outcome) (safe : .fatal ∉ family) :
    Admissible family :=
  safe

/-- A family whose every substitution divides by zero is admitted, with no
instance, and warns. -/
theorem entirely_undefined_warned (family : List Outcome)
    (undefined : ∀ outcome ∈ family, outcome = .zeroDivisor) (nonempty : family ≠ []) :
    Warned family := by
  refine ⟨?_, ?_⟩
  · intro failed
    exact Outcome.noConfusion (undefined .fatal failed)
  · cases family with
    | nil => exact absurd rfl nonempty
    | cons head tail =>
      have first : head = .zeroDivisor := undefined head (by simp)
      simp [first]

/-- A defined instance neither causes nor prevents the warning: a family
warns exactly when it is admitted and holds a zero divisor. -/
theorem warning_ignores_defined (family : List Outcome) (outcome : Outcome)
    (defined : outcome = .produced ∨ outcome = .filtered) :
    Warned (outcome :: family) ↔ Warned family := by
  cases defined with
  | inl produced => simp [Warned, Admissible, produced]
  | inr filtered => simp [Warned, Admissible, filtered]

/-- Reordering or revisiting substitutions preserves admission when it preserves
the classified outcomes. Completeness of that coverage is a caller obligation. -/
theorem admission_by_outcomes (before after : List Outcome)
    (same : ∀ outcome, outcome ∈ before ↔ outcome ∈ after) :
    Admissible before ↔ Admissible after := by
  simp only [Admissible, same]

/-- The same coverage relation preserves whether a successful family warns. -/
theorem warning_by_outcomes (before after : List Outcome)
    (same : ∀ outcome, outcome ∈ before ↔ outcome ∈ after) :
    Warned before ↔ Warned after := by
  have admission : Admissible before ↔ Admissible after :=
    admission_by_outcomes before after same
  simp only [Warned, admission, same]

/-- Admission is pointwise in any partition of a family: flattening local
families, or splitting one, changes no verdict. -/
theorem admission_of_append (first second : List Outcome) :
    Admissible (first ++ second) ↔ Admissible first ∧ Admissible second := by
  simp [Admissible, not_or]

/-- A refusal can be read from a prefix: nothing that follows a fatal failure
admits its family. -/
theorem refusal_from_prefix (first second : List Outcome) (refused : ¬ Admissible first) :
    ¬ Admissible (first ++ second) := by
  intro admitted
  exact refused ((admission_of_append first second).mp admitted).1

/-- Admission cannot be read from a prefix: a later substitution can fail
fatally. A family is admitted only on complete evidence. -/
theorem prefix_does_not_establish_admission :
    Admissible [.zeroDivisor] ∧ ¬ Admissible [.zeroDivisor, .fatal] := by
  simp [Admissible]

/-- A warning is raised by either part of an admitted family, so one raised
for a local family stands for the occurrence that holds it. -/
theorem warning_of_append (first second : List Outcome) :
    Warned (first ++ second) ↔
      Admissible (first ++ second) ∧ (.zeroDivisor ∈ first ∨ .zeroDivisor ∈ second) := by
  simp [Warned]

/-- The consequent of a body conditional at one condition binding: some
alternative is defined and holds. `none` is an alternative that divides by
zero. -/
def Consequent (alternatives : List (Option Prop)) : Prop :=
  ∃ value, some value ∈ alternatives ∧ value

/-- A consequent with no defined alternative is false, whatever sign its
literal has. -/
theorem undefined_consequent_false (alternatives : List (Option Prop))
    (undefined : ∀ alternative ∈ alternatives, alternative = none) :
    ¬ Consequent alternatives := by
  rintro ⟨value, member, _⟩
  exact Option.some_ne_none value (undefined (some value) member)

/-- The instance of such a consequent holds exactly when its condition does
not: it is the instance of `#false : condition`. -/
theorem undefined_consequent_excludes_condition (condition : Prop)
    (alternatives : List (Option Prop))
    (undefined : ∀ alternative ∈ alternatives, alternative = none) :
    (condition → Consequent alternatives) ↔ ¬ condition := by
  constructor
  · intro implication holds
    exact undefined_consequent_false alternatives undefined (implication holds)
  · intro absent holds
    exact absurd holds absent

/-- An undefined alternative beside others changes nothing: the defined ones
decide the consequent. -/
theorem undefined_alternative_ignored (alternatives : List (Option Prop)) :
    Consequent (none :: alternatives) ↔ Consequent alternatives := by
  simp [Consequent]

/-- The head of a conditional disjunct at one condition binding: every value
its head takes holds. -/
def Head (instances : List (Option Prop)) : Prop :=
  ∀ value, some value ∈ instances → value

/-- A head with no defined instance holds. -/
theorem undefined_head_true (instances : List (Option Prop))
    (undefined : ∀ occurrence ∈ instances, occurrence = none) :
    Head instances := by
  intro value member
  exact absurd (undefined (some value) member) (Option.some_ne_none value)

/-- Its disjunct holds exactly when the condition does: it is the disjunct of
`#true : condition`. -/
theorem undefined_head_disjunct (condition : Prop) (instances : List (Option Prop))
    (undefined : ∀ occurrence ∈ instances, occurrence = none) :
    (condition ∧ Head instances) ↔ condition := by
  constructor
  · exact And.left
  · intro holds
    exact ⟨holds, undefined_head_true instances undefined⟩

/-- A link of a comparison chain at one substitution: `some truth` when both
sides are defined, `none` when a side divides by zero. -/
abbrev Link := Option Bool

/-- The value of a chain from its links, as clingo grounds them: separate
comparisons, so that a link which is defined and false decides the chain
even beside a link that divides by zero. `some truth` is a defined chain and
`none` an undefined one, whose instance is omitted with a warning. -/
def chain (links : List Link) : Option Bool :=
  if some false ∈ links then some false
  else if none ∈ links then none
  else some true

/-- A false link decides the chain, whatever its other links are. -/
theorem false_link_decides (links : List Link) (failed : some false ∈ links) :
    chain links = some false := by
  simp [chain, failed]

/-- A chain is undefined exactly when no link is false and one divides by
zero. -/
theorem chain_undefined_iff (links : List Link) :
    chain links = none ↔ some false ∉ links ∧ none ∈ links := by
  unfold chain
  by_cases failed : some false ∈ links
  · simp [failed]
  · by_cases undefined : none ∈ links <;> simp [failed, undefined]

/-- A chain holds exactly when every link is defined and holds. -/
theorem chain_holds_iff (links : List Link) :
    chain links = some true ↔ ∀ link ∈ links, link = some true := by
  unfold chain
  by_cases failed : some false ∈ links
  · simp only [failed, if_true]
    constructor
    · intro impossible
      cases impossible
    · intro every
      cases every (some false) failed
  · by_cases undefined : none ∈ links
    · simp only [failed, undefined, if_false, if_true]
      constructor
      · intro impossible
        cases impossible
      · intro every
        cases every none undefined
    · simp only [failed, undefined, if_false, true_iff]
      intro link member
      match link, member with
      | some true, _ => rfl
      | some false, member => exact absurd member failed
      | none, member => exact absurd member undefined

/-- The value does not depend on the order of the links or on repeats. -/
theorem chain_by_links (before after : List Link)
    (same : ∀ link, link ∈ before ↔ link ∈ after) :
    chain before = chain after := by
  simp only [chain, same]

/-- A negated chain is the complement of a defined chain and undefined with
it: `not` of a chain a false link decides holds. -/
def negated (links : List Link) : Option Bool :=
  (chain links).map not

theorem negated_false_link_holds (links : List Link) (failed : some false ∈ links) :
    negated links = some true := by
  simp [negated, false_link_decides links failed]

/-- The source classifier records every construct which can create independent
nested-scope tasks. A false classification is uniform in the outer binding;
an empty result for one binding alone is not this premise. -/
theorem absent_nested_outcomes {Literal Binding : Type}
    (nested : Literal → Bool) (expand : Literal → Binding → List Outcome)
    (sound : ∀ literal, nested literal = false → ∀ binding, expand literal binding = [])
    (body : List Literal) (absent : body.any nested = false) (binding : Binding) :
    body.flatMap (fun literal => expand literal binding) = [] := by
  induction body with
  | nil => rfl
  | cons head tail ih =>
    have parts : nested head = false ∧ tail.any nested = false := by
      simpa using absent
    simp [sound head parts.1 binding, ih parts.2]

/-- Omitting a uniformly empty nested-outcome family changes neither admission
nor zero-divisor warnings. The caller still validates the enclosing row and
its independent scalar fields. No resource or polling equivalence is claimed. -/
theorem absent_nested_preserves_family (family nested : List Outcome)
    (absent : nested = []) :
    (Admissible (family ++ nested) ↔ Admissible family) ∧
      (Warned (family ++ nested) ↔ Warned family) := by
  simp [absent]

end Zetesis.ArithmeticFamilies
