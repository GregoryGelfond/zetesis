import Zetesis.CountEligibility

/-!
# Head permission and candidate measures

A finite aggregate head separates rules permitting its positive atoms from an
activated numeric constraint. Coalesced eligibility remains a formula, including
its frozen reduct. A zero numeric contribution does not remove that permission.
The numeric guard, in contrast, is inspected only in the original candidate.

The laws below compose eligibility equivalence and original numeric agreement
into equivalence of the complete head group, including arbitrary frozen M/J
interpretations and stable models in context. They do not derive a sum's numeric
meaning: tuple coverage, tuple/atom correspondence, signed-weight normalization,
finite-width arithmetic and source aggregate semantics remain separate premises.
In particular these laws do not resolve negative #sum+ head behavior or verify
the Rust compiler, cursor, resource accounting or GPU execution.
-/

namespace Zetesis.HeadMeasures
open Ferraris ChoiceIntervals
universe u
variable {A : Type u}

/-- Eligibility permits a head; neither eligibility nor a weight asserts it. -/
def permission (body eligible : Formula A) (head : A) : Formula A :=
  .imp (.conj body eligible) (.disj (.atom head) (Neg (.atom head)))

/-- A false measure guard rejects an active body without providing support. -/
def bound (body measure : Formula A) : Formula A :=
  Neg (.conj body (Neg measure))

/-- Original bound truth is the implication from activation to the measure. -/
theorem bound_original (M : Atoms A) (body measure : Formula A) :
    Satisfies M (bound body measure) ↔
      (Satisfies M body → Satisfies M measure) := by
  classical
  simp [bound, Ferraris.Neg, Satisfies]

/-- A bound's reduct inspects only M, never supplying support to J. -/
theorem bound_frozen (M J : Atoms A) (body measure : Formula A) :
    Satisfies J (Reduct M (bound body measure)) ↔
      (Satisfies M body → Satisfies M measure) := by
  classical
  rw [bound, negation_frozen]
  simp [Ferraris.Neg, Satisfies]

/-- Numeric agreement in every original interpretation suffices for the
    activated constraint. Frozen numeric equivalence is not required here. -/
theorem bound_equivalent (body first second : Formula A)
    (numeric : ∀ M, Satisfies M first ↔ Satisfies M second) :
    Equivalent (bound body first) (bound body second) := by
  constructor
  · intro M
    rw [bound_original, bound_original, numeric M]
  · intro M J
    rw [bound_frozen, bound_frozen, numeric M]

/-- Original/frozen eligibility equivalence preserves permission. -/
theorem permission_equivalent (body first second : Formula A) (head : A)
    (eligibility : Equivalent first second) :
    Equivalent (permission body first head) (permission body second head) := by
  exact equivalent_imp _ _ _ _
    (equivalent_conj _ _ _ _ (equivalent_refl body) eligibility)
    (equivalent_refl _)

/-- All permissions are retained independently of the numeric guard's inputs.
    A complete carrier can therefore contain zero-weight heads. -/
def group (body : Formula A) (heads : List A) (eligible : A → Formula A)
    (measure : Formula A) : Formula A :=
  .conj (all (heads.map (fun head => permission body (eligible head) head)))
    (bound body measure)

/-- Preserving each original eligibility/reduct and the original numeric
    function preserves the complete head group. Carrier completeness and
    correct numeric compilation must be established by their own arguments. -/
theorem group_equivalent (body : Formula A) (heads : List A)
    (first second : A → Formula A) (firstMeasure secondMeasure : Formula A)
    (eligibility : ∀ head, Equivalent (first head) (second head))
    (numeric : ∀ M, Satisfies M firstMeasure ↔ Satisfies M secondMeasure) :
    Equivalent (group body heads first firstMeasure)
      (group body heads second secondMeasure) := by
  have permissions : Equivalent
      (all (heads.map (fun head => permission body (first head) head)))
      (all (heads.map (fun head => permission body (second head) head))) := by
    apply all_map_equivalent
    intro head
    exact permission_equivalent body (first head) (second head) head (eligibility head)
  exact equivalent_conj _ _ _ _ permissions
    (bound_equivalent body firstMeasure secondMeasure numeric)

/-- Equivalent measured groups retain stable membership in any surrounding
    theory. Candidate generation cannot substitute for this original theory. -/
theorem stable_in_context (M : Atoms A) (body : Formula A) (heads : List A)
    (first second : A → Formula A) (firstMeasure secondMeasure : Formula A)
    (eligibility : ∀ head, Equivalent (first head) (second head))
    (numeric : ∀ candidate,
      Satisfies candidate firstMeasure ↔ Satisfies candidate secondMeasure)
    (context : Theory A) :
    Stable M (group body heads first firstMeasure :: context) ↔
      Stable M (group body heads second secondMeasure :: context) := by
  have preservation := group_equivalent body heads first second
    firstMeasure secondMeasure eligibility numeric
  simp only [Stable, models_cons, ReductTheory, List.map_cons,
    preservation.1, preservation.2]

end Zetesis.HeadMeasures
