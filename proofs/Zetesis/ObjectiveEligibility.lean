import Zetesis.ObjectiveConditions
import Std.Tactic

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

universe u

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

/-- Fold original Boolean formula operations without testing correlations
between atom occurrences. Implication is represented as negation/disjunction
by the existing original-query translation, never by a reduct rewrite. -/
def analyze {A : Type u} (atoms : A → Activity) : ObjectiveConditions.Query A → Activity
  | .boolean false => .absent
  | .boolean true => .required
  | .atom atom => atoms atom
  | .neg operand => negate (analyze atoms operand)
  | .conj left right => conjunction (analyze atoms left) (analyze atoms right)
  | .disj left right => disjunction (analyze atoms left) (analyze atoms right)

/-- Pointwise atom coverage suffices for coverage of every finite original
formula query. The proof follows its constructors and reuses the three
operation laws; it requires no independence or realization assumption. -/
theorem analyze_covers {A : Type u} (atoms : A → Activity) (model : A → Bool)
    (query : ObjectiveConditions.Query A)
    (covered : ∀ atom, Covers (atoms atom) (model atom)) :
    Covers (analyze atoms query) (ObjectiveConditions.evaluate model query) := by
  induction query with
  | boolean value => cases value <;> rfl
  | atom atom => exact covered atom
  | neg operand ih => exact negation_covers _ _ ih
  | conj left right first second => exact conjunction_covers _ _ _ _ first second
  | disj left right first second => exact disjunction_covers _ _ _ _ first second

/-- Refinement may resolve an optional atom, but retains already established
information. A runtime consumer separately checks consistent proposals. -/
def refine : Activity → Activity → Activity
  | .optional, proposed => proposed
  | known, _ => known

/-- Retaining known information or adopting a covered proposal preserves
coverage of the same original truth. No new candidate is chosen. -/
theorem refinement_covers (old proposed : Activity) (truth : Bool)
    (oldCovered : Covers old truth) (proposedCovered : Covers proposed truth) :
    Covers (refine old proposed) truth := by
  cases old with
  | absent => exact oldCovered
  | optional => exact proposedCovered
  | required => exact oldCovered

/-- Count the still-optional entries in a finite activity carrier. Each resolved
entry removes one uncertainty, independently of whether it becomes absent or
required. This supplies the decreasing measure for complete changing rounds. -/
def unknowns : List Activity → Nat
  | [] => 0
  | .optional :: rest => 1 + unknowns rest
  | _ :: rest => unknowns rest

/-- Paired old/new lists retain exact atom positions. A complete refinement
round cannot increase the number of optional entries. -/
theorem refinement_unknowns_le (pairs : List (Activity × Activity)) :
    unknowns (pairs.map (fun pair => refine pair.1 pair.2)) ≤
      unknowns (pairs.map Prod.fst) := by
  induction pairs with
  | nil => exact Nat.le_refl 0
  | cons pair rest ih =>
    rcases pair with ⟨old, proposed⟩
    cases old <;> cases proposed <;> simp_all [refine, unknowns] <;> omega

/-- A round that resolves at least one optional entry strictly decreases a
natural-number measure. Thus complete changing rounds cannot continue forever
on one finite atom carrier; a final unchanged round establishes termination.
This does not prove the Rust producer traversal or its carrier correspondence. -/
theorem changing_round_decreases (pairs : List (Activity × Activity))
    (changed : ∃ pair ∈ pairs, pair.1 = .optional ∧ pair.2 ≠ .optional) :
    unknowns (pairs.map (fun pair => refine pair.1 pair.2)) <
      unknowns (pairs.map Prod.fst) := by
  induction pairs with
  | nil => simp at changed
  | cons pair rest ih =>
    have bound :
        unknowns (rest.map (fun pair => refine pair.1 pair.2)) ≤
          unknowns (rest.map Prod.fst) := by
      exact refinement_unknowns_le rest
    obtain ⟨selected, member, optional, resolved⟩ := changed
    cases List.mem_cons.mp member with
    | inl same =>
      subst selected
      rcases pair with ⟨old, proposed⟩
      cases old <;> cases proposed <;> simp_all [refine, unknowns] <;> omega
    | inr inside =>
      have strict :
          unknowns (rest.map (fun pair => refine pair.1 pair.2)) <
            unknowns (rest.map Prod.fst) := by
        exact ih ⟨selected, inside, optional, resolved⟩
      rcases pair with ⟨old, proposed⟩
      cases old <;> cases proposed <;> simp_all [refine, unknowns] <;> omega

end Zetesis.ObjectiveEligibility
