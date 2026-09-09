import Zetesis.NormalFerraris

/-!
# A checked consumer of the normalized/Ferraris bridge

The manual's two-rule program is `a :- not b. b :- not a.`. This example proves
that {a} is an answer set by computing its least reduct closure and applying the
existing semantic correspondence. It proves one candidate's membership, not a
complete world view or a refinement of the Rust source parser or evaluator.

The anchors are included directly in the mdBook. This module is imported by the
checked umbrella, so its displayed declarations are compiled and axiom-audited
with the mathematical library.
-/

namespace Zetesis.Examples.Choices

open Semantics

-- ANCHOR: choices_program
inductive Atom where
  | a
  | b
  deriving DecidableEq

def program : Program Atom :=
  [⟨some .a, [], [], [.b], True⟩,
   ⟨some .b, [], [], [.a], True⟩]

def candidate : Atoms Atom := fun atom => atom = .a
-- ANCHOR_END: choices_program

-- ANCHOR: choices_closure
/-- Freezing the two rules at {a} leaves exactly the consequence a. -/
theorem candidate_closure : Gamma program candidate = candidate := by
  have consequence (X : Atoms Atom) (atom : Atom) :
      Consequence program candidate X atom ↔ candidate atom := by
    cases atom <;> simp [Consequence, program, Gate, Body, candidate]
  have closed : Closed (Consequence program candidate) candidate := by
    intro atom derived
    exact (consequence candidate atom).mp derived
  have generated : Sub candidate (Gamma program candidate) := by
    intro atom present
    exact gamma_closed program candidate atom
      ((consequence (Gamma program candidate) atom).mpr present)
  exact sub_antisymm (gamma_le closed) generated
-- ANCHOR_END: choices_closure

-- ANCHOR: choices_answer_set
/-- The existing closure theorem establishes membership under Ferraris semantics. -/
theorem candidate_answer_set : Ferraris.Stable candidate (NormalFerraris.translate program) := by
  have constraints : ConstraintsOK program candidate candidate := by
    simp [ConstraintsOK, program]
  exact (NormalFerraris.ferraris_answer_set_iff_closure program candidate).mpr
    ⟨candidate_closure, constraints⟩
-- ANCHOR_END: choices_answer_set

end Zetesis.Examples.Choices
