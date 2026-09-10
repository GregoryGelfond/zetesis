import Zetesis.Ferraris
import Zetesis.IntegerEnvelopes

/-!
# Finite-domain contraction inside an answer-set branch

Domains restrict a fixed assignment reading of answer sets of the original
Ferraris theory. Compatible completions also extend a partial assignment and
satisfy fixed activation assumptions. Recognition must establish that every
such active answer set obeys the proposed assignment constraints. A finite
filter may then remove values only when it preserves all satisfying assignments.

These laws connect a constraint filter to answer-set completions; they do not
recognize exactly-one source groups or prove that an assignment reading denotes
those groups. That mandatory-function correspondence, activation, checked
integer bounds, runtime filter refinement, resource accounting and implementation
refinement remain explicit external obligations. These laws justify filtering
after source admission. Filtering before grounding additionally requires that
skipped source evaluations are defined, or that the original arithmetic/error
admission boundary has already been preserved. Answer-set retention alone does
not justify suppressing a source error. The theory and its reduct do not change. Nonempty domains and a filtering fixed point do not establish
support, minimality or the existence of an answer set. See Propagation's existing
quiescent-unsatisfiable and unsupported-self-implication counterexamples.
-/

namespace Zetesis.DomainContraction

universe u
variable {Atom : Type u} {variables : Nat}

/-- A total assignment to the recognized mandatory finite-domain groups. -/
abbrev Assignment (variables : Nat) := Fin variables → Int

/-- Finite candidate values for each recognized group. List order and duplicate
occurrences do not alter membership in the denotational completion family. -/
abbrev Domains (variables : Nat) := Fin variables → List Int

/-- Every chosen value belongs to its group's current finite domain. -/
def Fits (domains : Domains variables) (assignment : Assignment variables) : Prop :=
  ∀ group, assignment group ∈ domains group

/-- Every already assigned group retains its exact chosen value. -/
def Extends (given : Fin variables → Option Int) (assignment : Assignment variables) : Prop :=
  ∀ group value, given group = some value → assignment group = value

/-- Completion means an answer set of the original theory, within this activated
branch and its domains. The fixed reading must correspond to the source groups;
an arbitrary classical valuation cannot stand in for this answer set. -/
def Compatible (theory : Ferraris.Theory Atom) (active : Atoms Atom → Prop)
    (read : Atoms Atom → Assignment variables) (given : Fin variables → Option Int)
    (domains : Domains variables) (answerSet : Atoms Atom) : Prop :=
  Ferraris.Stable answerSet theory ∧ active answerSet ∧
    Extends given (read answerSet) ∧ Fits domains (read answerSet)

/-- Recognition must justify the constraints for every active answer set under
the fixed source-group reading. Conditional constraints need their activation
assumptions here; mere syntactic resemblance cannot establish this property. -/
def Recognizes (theory : Ferraris.Theory Atom) (active : Atoms Atom → Prop)
    (read : Atoms Atom → Assignment variables) (constraints : Assignment variables → Prop) : Prop :=
  ∀ answerSet, Ferraris.Stable answerSet theory → active answerSet →
    constraints (read answerSet)

/-- A finite-domain filter computes a keep predicate from one unchanged domain
snapshot. It does not decide whether the theory has an answer set. -/
abbrev Filter (variables : Nat) := Domains variables → Fin variables → Int → Bool

/-- Apply the same snapshot to every group, then publish the filtered domains. -/
def contract (filter : Filter variables) (domains : Domains variables) : Domains variables :=
  fun group => (domains group).filter (filter domains group)

/-- A sound filter retains the chosen value of every compatible assignment.
Arithmetic or distinctness implementations must establish this premise. -/
def Preserves (constraints : Assignment variables → Prop) (filter : Filter variables) : Prop :=
  ∀ domains assignment, Fits domains assignment → constraints assignment →
    ∀ group, filter domains group (assignment group) = true

/-- Listed mandatory groups choose pairwise distinct values. -/
def Distinct (groups : List (Fin variables)) (assignment : Assignment variables) : Prop :=
  ∀ group ∈ groups, ∀ other ∈ groups, group ≠ other → assignment group ≠ assignment other

/-- Remove a singleton value from each other group in a distinctness constraint.
Groups outside that constraint are unchanged. All singleton reads use the same
input domain snapshot. -/
def singletonFilter (groups : List (Fin variables)) : Filter variables :=
  fun domains group value =>
    if group ∈ groups then
      groups.all fun other =>
        if other = group then true
        else match domains other with
          | [chosen] => decide (value ≠ chosen)
          | _ => true
    else true

/-- Singleton distinctness filtering retains every fitting distinct assignment.
A fitting assignment must choose the singleton's sole value. Distinctness then
excludes that value at every different participating group. -/
theorem singleton_filter_preserves (groups : List (Fin variables)) :
    Preserves (Distinct groups) (singletonFilter groups) := by
  intro domains assignment fits distinct group
  by_cases participating : group ∈ groups
  · simp only [singletonFilter, if_pos participating]
    apply List.all_eq_true.mpr
    intro other present
    by_cases same : other = group
    · simp [same]
    · simp only [if_neg same]
      cases values : domains other with
      | nil => rfl
      | cons chosen rest =>
        cases rest with
        | nil =>
          have chosen_value : assignment other = chosen := by
            simpa [values] using fits other
          have different : assignment group ≠ chosen := by
            rw [← chosen_value]
            exact distinct group participating other present (Ne.symm same)
          simpa using different
        | cons next rest => rfl
  · simp [singletonFilter, participating]

/-- Sound integer envelopes for each finite domain. Endpoint construction is a
separate obligation; these fields do not assume source machine arithmetic is
unbounded. Empty domains need no selected-value bound. -/
structure Envelopes (variables : Nat) where
  lower : Domains variables → Fin variables → Int
  upper : Domains variables → Fin variables → Int
  encloses : ∀ domains group value, value ∈ domains group →
    lower domains group ≤ value ∧ value ≤ upper domains group

/-- Coefficient/group pairs retain repeated and correlated group occurrences. -/
abbrev Terms (variables : Nat) := List (Int × Fin variables)

/-- Mathematical affine remainder; normalization and machine overflow are not
part of this value. -/
def remainder (terms : Terms variables) (assignment : Assignment variables) : Int :=
  (terms.map fun term => term.1 * assignment term.2).sum

/-- Choose each coefficient's lower product endpoint, then add those bounds. -/
def lowerRemainder (envelopes : Envelopes variables) (domains : Domains variables)
    (terms : Terms variables) : Int :=
  (terms.map fun term => if 0 ≤ term.1 then term.1 * envelopes.lower domains term.2
    else term.1 * envelopes.upper domains term.2).sum

/-- The lower remainder bounds every fitting assignment, including correlated
or repeated groups. Apply the existing product law to each selected value, then
the existing finite-sum law; no independence assumption is needed. -/
theorem remainder_lower_bound (envelopes : Envelopes variables) (domains : Domains variables)
    (terms : Terms variables) (assignment : Assignment variables) (fits : Fits domains assignment) :
    lowerRemainder envelopes domains terms ≤ remainder terms assignment := by
  apply IntegerEnvelopes.sum_lower_bound terms
  intro term _
  exact IntegerEnvelopes.product_lower_bound term.1 (assignment term.2)
    (envelopes.lower domains term.2) (envelopes.upper domains term.2)
    (envelopes.encloses domains term.2 (assignment term.2) (fits term.2))

/-- One unconditional affine upper comparison under the recognized assignment
reading. An equality can supply both directed upper comparisons. -/
def Affine (target : Fin variables) (coefficient : Int) (terms : Terms variables)
    (bound : Int) (assignment : Assignment variables) : Prop :=
  coefficient * assignment target + remainder terms assignment ≤ bound

/-- Keep a target value when it passes the necessary upper-bound check formed
from the conservative remainder. Other groups are unchanged. Coefficients may be negative
or zero; no integer division or endpoint rounding occurs in this filter. -/
def affineFilter (envelopes : Envelopes variables) (target : Fin variables)
    (coefficient : Int) (terms : Terms variables) (bound : Int) : Filter variables :=
  fun domains group value => if group = target then
    decide (coefficient * value + lowerRemainder envelopes domains terms ≤ bound)
  else true

/-- Affine filtering retains every fitting solution of its comparison. The true
remainder is at least its conservative bound; replacing it by that lower bound
cannot turn a satisfied upper comparison into a rejection. -/
theorem affine_filter_preserves (envelopes : Envelopes variables) (target : Fin variables)
    (coefficient : Int) (terms : Terms variables) (bound : Int) :
    Preserves (Affine target coefficient terms bound)
      (affineFilter envelopes target coefficient terms bound) := by
  intro domains assignment fits valid group
  by_cases same : group = target
  · subst group
    have lower := remainder_lower_bound envelopes domains terms assignment fits
    have comparison : coefficient * assignment target + remainder terms assignment ≤ bound := valid
    have retained : coefficient * assignment target +
        lowerRemainder envelopes domains terms ≤ bound := by omega
    simpa [affineFilter] using retained
  · simp [affineFilter, same]

/-- Contraction only removes values; it cannot introduce a new completion. -/
theorem contraction_decreases (filter : Filter variables) (domains : Domains variables)
    (assignment : Assignment variables) (fits : Fits (contract filter domains) assignment) :
    Fits domains assignment := by
  intro group
  exact (List.mem_filter.mp (fits group)).1

/-- Recognition and filter soundness preserve exactly the compatible answer-set
family. Narrowing supplies the forward direction. For the reverse direction,
recognition supplies the constraints, and soundness retains every chosen value.
No consistency result is promoted to answer-set membership. -/
theorem compatible_contraction (theory : Ferraris.Theory Atom) (active : Atoms Atom → Prop)
    (read : Atoms Atom → Assignment variables) (given : Fin variables → Option Int)
    (constraints : Assignment variables → Prop) (filter : Filter variables)
    (recognized : Recognizes theory active read constraints)
    (preserves : Preserves constraints filter) (domains : Domains variables)
    (answerSet : Atoms Atom) :
    Compatible theory active read given (contract filter domains) answerSet ↔
      Compatible theory active read given domains answerSet := by
  constructor
  · rintro ⟨stable, activated, extended, fits⟩
    exact ⟨stable, activated, extended, contraction_decreases filter domains (read answerSet) fits⟩
  · rintro ⟨stable, activated, extended, fits⟩
    have valid : constraints (read answerSet) := recognized answerSet stable activated
    have retained : Fits (contract filter domains) (read answerSet) := by
      intro group
      exact List.mem_filter.mpr ⟨fits group, preserves domains (read answerSet) fits valid group⟩
    exact ⟨stable, activated, extended, retained⟩

/-- A finite list of filter applications. Its remaining list is the decreasing
measure; completion of this list does not claim propagation reached a fixed point. -/
def applyFilters : List (Filter variables) → Domains variables → Domains variables
  | [], domains => domains
  | filter :: rest, domains => applyFilters rest (contract filter domains)

/-- Any completed finite prefix of sound filters preserves the same compatible
answer-set family. The budget limits the number of applications, not internal
filter work. A runtime must separately account for interrupted applications. -/
theorem bounded_filters_preserve_completions (theory : Ferraris.Theory Atom)
    (active : Atoms Atom → Prop) (read : Atoms Atom → Assignment variables)
    (given : Fin variables → Option Int) (constraints : Assignment variables → Prop)
    (recognized : Recognizes theory active read constraints)
    (filters : List (Filter variables))
    (sound : ∀ filter ∈ filters, Preserves constraints filter)
    (budget : Nat) (domains : Domains variables) (answerSet : Atoms Atom) :
    Compatible theory active read given (applyFilters (filters.take budget) domains) answerSet ↔
      Compatible theory active read given domains answerSet := by
  have finite_preservation : ∀ (steps : List (Filter variables)),
      (∀ filter ∈ steps, Preserves constraints filter) → ∀ initial,
      Compatible theory active read given (applyFilters steps initial) answerSet ↔
        Compatible theory active read given initial answerSet := by
    intro steps
    induction steps with
    | nil => intro _ _; rfl
    | cons filter rest induction =>
      intro each initial
      have tail_sound : ∀ other ∈ rest, Preserves constraints other := by
        intro other member
        exact each other (by simp [member])
      have head_sound : Preserves constraints filter := each filter (by simp)
      change Compatible theory active read given
        (applyFilters rest (contract filter initial)) answerSet ↔ _
      rw [induction tail_sound,
        compatible_contraction theory active read given constraints filter recognized head_sound]
  apply finite_preservation (filters.take budget)
  intro filter member
  exact sound filter (List.mem_of_mem_take member)

/-- An empty mandatory domain rules out this certified branch's compatible
answer sets. It says nothing about branches where the activation assumptions
fail, and it does not turn nonempty domains into a membership decision. -/
theorem empty_domain_excludes_completions (theory : Ferraris.Theory Atom)
    (active : Atoms Atom → Prop) (read : Atoms Atom → Assignment variables)
    (given : Fin variables → Option Int) (domains : Domains variables)
    (group : Fin variables) (empty : domains group = []) :
    ¬ ∃ answerSet, Compatible theory active read given domains answerSet := by
  rintro ⟨answerSet, _, _, _, fits⟩
  have inside := fits group
  simp [empty] at inside

end Zetesis.DomainContraction
