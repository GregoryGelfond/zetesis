import Zetesis.ObjectiveConditions

/-!
# Source activity and model-relative conditions

An activity describes a source grounding abstraction: absent, optional or
required. Optional does not assert simultaneous satisfiability or answer-set
realization. The laws below establish coverage of original-model Boolean truth
under conjunction, disjunction and negation. They do not strengthen that coverage
into completeness of a source grounder's retained objective rows.

The runtime source profile must establish its atom classifications by completed
finite producer traversal or a complete possible-support carrier. The latter
can classify cyclic ordinary producers as optional without solving for truth.
Concrete support coverage, source simplification, aggregate carrier construction
and Rust resource completion remain separate obligations. In particular, an incomplete traversal supplies no activity result.
-/

namespace Zetesis.ObjectiveEligibility

/-- Optional is a grounding possibility, not a jointly realizable witness. -/
inductive Activity where
  | absent
  | optional
  | required
deriving DecidableEq

/-- Every actual Boolean truth must lie within the supplied source activity. -/
def Covers : Activity → Bool → Prop
  | .absent, truth => truth = false
  | .optional, _ => True
  | .required, truth => truth = true

def negate : Activity → Activity
  | .absent => .required
  | .optional => .optional
  | .required => .absent

def conjunction : Activity → Activity → Activity
  | .absent, _ | _, .absent => .absent
  | .required, .required => .required
  | _, _ => .optional

def disjunction : Activity → Activity → Activity
  | .required, _ | _, .required => .required
  | .absent, .absent => .absent
  | _, _ => .optional

/-- A possible-support carrier supplies no required truth: membership is
optional, while absence excludes truth only under its coverage premise. -/
def ofPossible (possible : Bool) : Activity :=
  if possible then .optional else .absent

/-- A completed upper carrier yields sound activity even for cyclic producers.
The proof needs only that every true atom belongs to that carrier; it makes no
acyclicity, realizability or exact source-grounding-retention assumption. -/
theorem possible_support_covers (possible truth : Bool)
    (covered : truth = true → possible = true) :
    Covers (ofPossible possible) truth := by
  cases possible <;> cases truth <;> simp_all [ofPossible, Covers]

/-- Complementing a source activity covers the complement of every covered
truth. Optional remains optional; no existence claim is introduced. -/
theorem negation_covers (activity : Activity) (truth : Bool)
    (covered : Covers activity truth) : Covers (negate activity) (!truth) := by
  cases activity <;> simp_all [Covers, negate]

/-- Conjunction preserves coverage even when the two original conditions are
correlated. The proof needs their pointwise coverage, not independence. -/
theorem conjunction_covers (left right : Activity) (first second : Bool)
    (leftCovered : Covers left first) (rightCovered : Covers right second) :
    Covers (conjunction left right) (first && second) := by
  cases left <;> cases right <;> simp_all [Covers, conjunction]

/-- Alternative source producers cover disjunction of their actual activity. -/
theorem disjunction_covers (left right : Activity) (first second : Bool)
    (leftCovered : Covers left first) (rightCovered : Covers right second) :
    Covers (disjunction left right) (first || second) := by
  cases left <;> cases right <;> simp_all [Covers, disjunction]

/-- Contradictory original conditions can retain a source possibility while
never enabling a contribution. Thus optional activity cannot prove realization. -/
theorem optional_contradiction (truth : Bool) :
    conjunction .optional (negate .optional) = .optional ∧
      (truth && !truth) = false := by
  cases truth <;> simp [conjunction, negate]

/-- An absent source condition cannot enable a model-relative contribution,
provided the source classifier established the stated coverage premise. -/
theorem absent_excludes (truth : Bool) (covered : Covers .absent truth) :
    truth = false := by
  exact covered

end Zetesis.ObjectiveEligibility
