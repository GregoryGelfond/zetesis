import Zetesis.ObjectiveEligibility

/-!
# Completed possible support and answer-set coverage

A source producer proposes possible atoms from an input carrier. Proposals may
be unrealizable: their purpose is coverage, not truth. A complete round that
adds no atom establishes closure under that producer. Monotonicity then puts
every finite generation stage inside the closed carrier; it does not establish
that generation reaches such a stage, or that a resource-limited attempt has
completed.

Answer-set coverage needs a separate semantic premise. `ProjectionCompatible`
says that intersecting any original model with a producer-closed carrier still
models the original model's frozen reduct. Minimality then excludes atoms
outside that carrier. This premise must be established for the source profile;
it is not a consequence of finite storage or an empty delta alone.

For the Rust adapter, ordinary positive joins supply necessary data bindings;
nonbinding aggregate/conditional truth never removes possible producers.
Aggregate assignment proposals retain complete keys and every possible value,
using `AggregateAssignment.covered_actual_sum_is_candidate` and
`ValueExtrema.completed_candidate_coverage`. Their original equalities remain in the theory.
These are the ingredients of the projection argument, not a formal refinement
of parsing, local binding plans, checked arithmetic or the Rust fixed-point
loop. An interrupted or resource-exhausted loop supplies no closure result.
-/

namespace Zetesis.SourceSupport

universe u
variable {A : Type u}

/-- A support round retains previous atoms and adds all current proposals. -/
def round (propose : Atoms A → Atoms A)
    (carrier : Atoms A) : Atoms A :=
  fun atom => carrier atom ∨ propose carrier atom

/-- Finite proposal stages start from the empty possible carrier. -/
def stage (propose : Atoms A → Atoms A) : Nat → Atoms A
  | 0 => fun _ => False
  | n + 1 => round propose (stage propose n)

/-- Every atom proposed from this carrier is already present. -/
def Closed (propose : Atoms A → Atoms A)
    (carrier : Atoms A) : Prop :=
  Sub (propose carrier) carrier

/-- Adding possible inputs cannot remove a possible proposal. -/
def Monotone (propose : Atoms A → Atoms A) : Prop :=
  ∀ left right, Sub left right →
    Sub (propose left) (propose right)

/-- A completed round with no new atom establishes producer closure. This is
an observed completion condition, not a bound on how many rounds are needed. -/
theorem unchanged_round_closed (propose : Atoms A → Atoms A)
    (carrier : Atoms A)
    (unchanged : Sub (round propose carrier) carrier) :
    Closed propose carrier := by
  intro atom proposed
  exact unchanged atom (Or.inr proposed)

/-- Every finite generation stage lies inside any producer-closed carrier,
provided proposal generation is monotone. The induction retains old atoms and
uses monotonicity to move each new proposal into the closed carrier. -/
theorem stages_inside_closed (propose : Atoms A → Atoms A)
    (monotone : Monotone propose) (carrier : Atoms A)
    (closed : Closed propose carrier) (n : Nat) :
    Sub (stage propose n) carrier := by
  induction n with
  | zero =>
    intro atom generated
    exact False.elim generated
  | succ n previous =>
    intro atom generated
    cases generated with
    | inl retained => exact previous atom retained
    | inr proposed =>
      have carrierProposal : propose carrier atom :=
        monotone (stage propose n) carrier previous atom proposed
      exact closed atom carrierProposal

/-- Retain only the atoms of an interpretation belonging to a possible carrier. -/
def restrict (model carrier : Atoms A) : Atoms A :=
  fun atom => model atom ∧ carrier atom

/-- The source-profile obligation connecting producer closure to its original
theory. Local binding and aggregate-value coverage must justify this erasure
property; the definition does not posit realizability of any proposal. -/
def ProjectionCompatible (propose : Atoms A → Atoms A)
    (theory : Ferraris.Theory A) : Prop :=
  ∀ carrier model, Closed propose carrier → Ferraris.Models model theory →
    Ferraris.Models (restrict model carrier) (Ferraris.ReductTheory model theory)

/-- Every atom in an answer set belongs to a closed possible carrier when the
source producer satisfies the reduct-projection premise. Otherwise the
intersection would be a smaller model of the same frozen reduct, contradicting
answer-set minimality. The original theory is never rewritten. -/
theorem stable_inside_closed (propose : Atoms A → Atoms A)
    (theory : Ferraris.Theory A) (compatible : ProjectionCompatible propose theory)
    (carrier model : Atoms A) (closed : Closed propose carrier)
    (stable : Ferraris.Stable model theory) : Sub model carrier := by
  have restrictedSubset : Sub (restrict model carrier) model := by
    intro atom retained
    exact retained.1
  have restrictedReduct :
      Ferraris.Models (restrict model carrier) (Ferraris.ReductTheory model theory) :=
    compatible carrier model closed stable.1
  have minimal : Ferraris.MinimalModel model (Ferraris.ReductTheory model theory) :=
    (Ferraris.stable_iff_minimal_reduct model theory).mp stable
  have allRetained : Sub model (restrict model carrier) :=
    minimal.2 (restrict model carrier) restrictedSubset restrictedReduct
  intro atom present
  exact (allRetained atom present).2

/-- The covered carrier supplies optional/absent source activity for each atom
of an answer set. Optional still says nothing about simultaneous realizability
or the exact priority slots reported by another grounder. -/
theorem completed_activity_covers (propose : Atoms A → Atoms A)
    (theory : Ferraris.Theory A) (compatible : ProjectionCompatible propose theory)
    (possible truth : A → Bool)
    (closed : Closed propose (fun atom => possible atom = true))
    (stable : Ferraris.Stable (fun atom => truth atom = true) theory) (atom : A) :
    ObjectiveEligibility.Covers (ObjectiveEligibility.ofPossible (possible atom))
      (truth atom) := by
  have covered : truth atom = true → possible atom = true :=
    stable_inside_closed propose theory compatible
      (fun value => possible value = true) (fun value => truth value = true)
      closed stable atom
  exact ObjectiveEligibility.possible_support_covers (possible atom) (truth atom) covered

end Zetesis.SourceSupport
