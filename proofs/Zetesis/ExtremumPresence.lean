import Zetesis.ValueExtrema
import Zetesis.ObjectiveTransport

/-!
# Numeric presence under mandatory extremum bounds

Possible values and mandatory values are completed grounding inputs. A numeric
presence witness is possible and is not dominated by a mandatory value. Optional
conditions need not be independent or jointly realizable; accepted models are
not an input to this presence predicate. The original candidate carrier remains
unchanged when its numeric presence witnesses are excluded.

The laws establish finite witness exclusion, optional-carrier monotonicity,
renaming transport and a semantic nonnumeric-extremum consequence under an
explicit ordered-selection law. They do not prove that this completed carrier
matches clingo, the Rust recognizer, source fact classification or term order.
Those correspondence obligations remain separate qualification boundaries.
-/

namespace Zetesis.ExtremumPresence

universe u v
variable {κ : Type u} {ψ : Type v}

/-- Priority presence consults the completed possible carrier and mandatory
    bounds, never the set of values realized in accepted models. -/
def NumericWitness (possible mandatory : List κ) (numeric : κ → Prop)
    (dominates : κ → κ → Prop) : Prop :=
  ∃ value, value ∈ possible ∧ numeric value ∧
    ∀ bound, bound ∈ mandatory → ¬ dominates bound value

theorem mandatory_dominance_excludes_numeric (possible mandatory : List κ)
    (numeric : κ → Prop) (dominates : κ → κ → Prop) (bound : κ)
    (required : bound ∈ mandatory)
    (dominance : ∀ value, numeric value → dominates bound value) :
    ¬ NumericWitness possible mandatory numeric dominates := by
  rintro ⟨value, _, number, retained⟩
  have excluded : dominates bound value := dominance value number
  have survives : ¬ dominates bound value := retained bound required
  exact survives excluded

/-- With no mandatory tuple, every possible numeric value witnesses presence,
    including values whose optional conditions cannot be jointly realized. -/
theorem optional_numeric_witness (possible : List κ) (numeric : κ → Prop)
    (dominates : κ → κ → Prop) :
    NumericWitness possible [] numeric dominates ↔ ∃ value ∈ possible, numeric value := by
  constructor
  · rintro ⟨value, member, number, _⟩
    exact ⟨value, member, number⟩
  · rintro ⟨value, member, number⟩
    exact ⟨value, member, number, by simp⟩

theorem possible_extension_retains_presence (before after mandatory : List κ)
    (numeric : κ → Prop) (dominates : κ → κ → Prop)
    (coverage : ∀ value ∈ before, value ∈ after)
    (present : NumericWitness before mandatory numeric dominates) :
    NumericWitness after mandatory numeric dominates := by
  obtain ⟨value, member, number, retained⟩ := present
  exact ⟨value, coverage value member, number, retained⟩

/-- Exact unary renaming transports the already completed witness predicate;
    preserving original activation remains a distinct obligation. -/
theorem numeric_presence_transport (rows : List κ) (rename : κ → ψ)
    (before : κ → Prop) (after : ψ → Prop)
    (same : ∀ row, after (rename row) ↔ before row) :
    ObjectiveTransport.Present (rows.map rename) after ↔
      ObjectiveTransport.Present rows before := by
  exact ObjectiveTransport.present_transport rows rename before after same

/-- A required tuple in a dominating nonnumeric class forces the selected
    extremum into that class. The ordering law is explicit rather than inferred
    from the Rust comparator or from a numeric sentinel. -/
theorem mandatory_class_excludes_numeric_result
    (before : κ → κ → Bool) (dominating numeric : κ → Prop)
    (selection : ∀ left right,
      dominating (ValueExtrema.choose before left right) ↔
        dominating left ∨ dominating right)
    (nonnumeric : ∀ value, dominating value → ¬ numeric value)
    (actual : List κ) (required result : κ)
    (member : required ∈ actual) (mandatory : dominating required)
    (selected : ValueExtrema.extreme before actual = some result) :
    ¬ numeric result := by
  have realized : ∃ value ∈ actual, dominating value :=
    ⟨required, member, mandatory⟩
  have belongs : dominating result :=
    (ValueExtrema.selected_predicate before dominating selection actual result selected).mpr realized
  exact nonnumeric result belongs

end Zetesis.ExtremumPresence
