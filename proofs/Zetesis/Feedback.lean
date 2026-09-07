import Zetesis.Ferraris

/-!
# Fixed-countermodel feedback and conditional cone exclusion

The residual formula retains each original subformula as the future candidate's
truth mask, while fixing the interpretation tested against its reduct. Its
classical truth is exactly that fixed witness's reduct truth, for arbitrary
candidates and witnesses. Feedback restricts candidates; it never changes the
original theory used for stable-model acceptance.

The atom carrier and original theory are fixed throughout. These denotational
definitions do not implement a finite proper-subset circuit, shared DAG compiler,
SAT restart, witness validation, resource bounds or program-version registry.
Cone exclusion below requires an explicit classical-minimality premise. No
source-fragment recognizer or splitting theorem is established here.
-/

namespace Zetesis.Feedback

open Ferraris

universe u
variable {α : Type u}

open Classical in
/-- Fix J, retaining each original binary formula as the truth mask at X. -/
noncomputable def Witness (J : Atoms α) : Formula α → Formula α
  | .atom a => if J a then .atom a else .bot
  | .bot => .bot
  | .conj F G => .conj (.conj F G) (.conj (Witness J F) (Witness J G))
  | .disj F G => .conj (.disj F G) (.disj (Witness J F) (Witness J G))
  | .imp F G => .conj (.imp F G) (.imp (Witness J F) (Witness J G))

/-- No subset assumption is needed for the formula transform itself. -/
theorem witness_exact (J X : Atoms α) (F : Formula α) :
    Satisfies X (Witness J F) ↔ Satisfies J (Reduct X F) := by
  classical
  induction F with
  | atom a =>
    by_cases hJ : J a <;> by_cases hX : X a <;> simp [Witness, Reduct, Satisfies, hJ, hX]
  | bot => rfl
  | conj F G ihF ihG =>
    by_cases hF : Satisfies X F <;> by_cases hG : Satisfies X G
      <;> simp_all [Witness, Reduct, Satisfies]
  | disj F G ihF ihG =>
    by_cases hF : Satisfies X F <;> by_cases hG : Satisfies X G
      <;> simp_all [Witness, Reduct, Satisfies]
  | imp F G ihF ihG =>
    by_cases hF : Satisfies X F <;> by_cases hG : Satisfies X G
      <;> simp_all [Witness, Reduct, Satisfies]

noncomputable def WitnessTheory (J : Atoms α) (T : Theory α) : Theory α :=
  T.map (Witness J)

theorem witness_theory_exact (J X : Atoms α) (T : Theory α) :
    Models X (WitnessTheory J T) ↔ Models J (ReductTheory X T) := by
  constructor
  · intro h F hF
    obtain ⟨G, hG, rfl⟩ := List.mem_map.mp hF
    exact (witness_exact J X G).mp (h (Witness J G) (List.mem_map.mpr ⟨G, hG, rfl⟩))
  · intro h F hF
    obtain ⟨G, hG, rfl⟩ := List.mem_map.mp hF
    exact (witness_exact J X G).mpr (h (Reduct X G) (List.mem_map.mpr ⟨G, hG, rfl⟩))

/-- A classical candidate restriction, not an additional original source root. -/
def Allow (J : Atoms α) (T : Theory α) (X : Atoms α) : Prop :=
  ¬ (ProperSub J X ∧ Models X (WitnessTheory J T))

theorem allow_iff_no_countermodel (J X : Atoms α) (T : Theory α) :
    Allow J T X ↔ ¬ (ProperSub J X ∧ Models J (ReductTheory X T)) := by
  unfold Allow
  rw [witness_theory_exact]

theorem stable_allows (J X : Atoms α) (T : Theory α) (stable : Stable X T) :
    Allow J T X := by
  rw [allow_iff_no_countermodel]
  intro witness
  exact stable.2 ⟨J, witness⟩

theorem feedback_preserves_stability (J X : Atoms α) (T : Theory α) :
    (Stable X T ∧ Allow J T X) ↔ Stable X T := by
  exact ⟨And.left, fun stable => ⟨stable, stable_allows J X T stable⟩⟩

/-- A proper-subset reduct model excludes its candidate, without needing to
    assume that the candidate was a classical model in the first place. -/
theorem countermodel_excludes_candidate (J M : Atoms α) (T : Theory α)
    (proper : ProperSub J M) (witness : Models J (ReductTheory M T)) :
    ¬ Allow J T M := by
  intro allowed
  exact (allow_iff_no_countermodel J M T).mp allowed ⟨proper, witness⟩

/-- Feedback's rejected region is exact, not merely a superset cone of J. -/
theorem excluded_has_countermodel (J X : Atoms α) (T : Theory α) :
    ¬ Allow J T X ↔ ProperSub J X ∧ Models J (ReductTheory X T) := by
  classical
  rw [allow_iff_no_countermodel]
  exact ⟨Classical.byContradiction, fun witness absent => absent witness⟩

def Allows (witnesses : List (Atoms α)) (T : Theory α) (X : Atoms α) : Prop :=
  ∀ J, J ∈ witnesses → Allow J T X

theorem stable_allows_all (witnesses : List (Atoms α)) (X : Atoms α) (T : Theory α)
    (stable : Stable X T) : Allows witnesses T X := by
  intro J _
  exact stable_allows J X T stable

theorem all_feedback_preserves_stability (witnesses : List (Atoms α))
    (X : Atoms α) (T : Theory α) :
    (Stable X T ∧ Allows witnesses T X) ↔ Stable X T := by
  exact ⟨And.left, fun stable => ⟨stable, stable_allows_all witnesses X T stable⟩⟩

theorem recorded_countermodel_excludes_candidate (witnesses : List (Atoms α))
    (J M : Atoms α) (T : Theory α) (recorded : J ∈ witnesses)
    (proper : ProperSub J M) (witness : Models J (ReductTheory M T)) :
    ¬ Allows witnesses T M := by
  intro allowed
  exact countermodel_excludes_candidate J M T proper witness (allowed J recorded)

/-- The single-atom choice formula admits both interpretations of Unit. -/
def UnitChoice : Theory Unit := [.disj (.atom ()) (Neg (.atom ()))]

theorem unit_choice_reduct_models (M J : Atoms Unit) :
    Models J (ReductTheory M UnitChoice) ↔ (M () → J ()) := by
  simp only [Models, ReductTheory, UnitChoice, List.map_cons, List.map_nil,
    List.mem_singleton, forall_eq]
  exact choice_reduct_guard M J ()

theorem unit_choice_empty_stable : Stable (Empty : Atoms Unit) UnitChoice := by
  constructor
  · intro F hF
    have same : F = .disj (.atom ()) (Neg (.atom ())) := by
      simpa [UnitChoice] using hF
    subst F
    exact Or.inr (fun impossible => impossible)
  · rintro ⟨J, proper, _⟩
    exact proper.2 (fun _ impossible => False.elim impossible)

theorem unit_choice_full_stable : Stable (Full : Atoms Unit) UnitChoice := by
  constructor
  · intro F hF
    have same : F = .disj (.atom ()) (Neg (.atom ())) := by
      simpa [UnitChoice] using hF
    subst F
    exact Or.inl True.intro
  · rintro ⟨J, proper, witness⟩
    have unit : J () := (unit_choice_reduct_models Full J).mp witness True.intro
    apply proper.2
    intro a _
    cases a
    exact unit

/-- Checked comparable stable models invalidate unqualified antichain pruning. -/
theorem unit_choice_comparable_stable_models :
    Stable (Empty : Atoms Unit) UnitChoice ∧ Stable (Full : Atoms Unit) UnitChoice ∧
    ProperSub (Empty : Atoms Unit) Full := by
  refine ⟨unit_choice_empty_stable, unit_choice_full_stable, ?_, ?_⟩
  · exact fun _ impossible => False.elim impossible
  · intro backward
    exact backward () True.intro

/-- Explicit semantic premise; recognizing any source fragment is separate. -/
def ClassicallyMinimalStable (T : Theory α) : Prop :=
  ∀ M, Stable M T → MinimalModel M T

theorem unit_choice_not_classically_minimal : ¬ ClassicallyMinimalStable UnitChoice := by
  intro premise
  have minimal := premise Full unit_choice_full_stable
  have backward := minimal.2 Empty (fun _ impossible => False.elim impossible)
    unit_choice_empty_stable.1
  exact backward () True.intro

theorem classical_minimal_stable_sub_equal (T : Theory α)
    (premise : ClassicallyMinimalStable T) (S X : Atoms α)
    (stableS : Stable S T) (stableX : Stable X T) (included : Sub S X) : S = X := by
  exact sub_antisymm included ((premise X stableX).2 S included stableS.1)

/-- Both cones include S itself. Its delivery/accounting is an external premise
    of using this guard to continue enumeration after S has been verified. -/
def OutsideCones (S X : Atoms α) : Prop := ¬ Sub X S ∧ ¬ Sub S X

theorem other_stable_outside_cones (T : Theory α)
    (premise : ClassicallyMinimalStable T) (S X : Atoms α)
    (stableS : Stable S T) (stableX : Stable X T) (different : X ≠ S) :
    OutsideCones S X := by
  constructor
  · intro included
    exact different (classical_minimal_stable_sub_equal T premise X S stableX stableS included)
  · intro included
    exact different (classical_minimal_stable_sub_equal T premise S X stableS stableX included).symm

theorem cone_filter_exact (T : Theory α) (premise : ClassicallyMinimalStable T)
    (S X : Atoms α) (stableS : Stable S T) :
    (Stable X T ∧ OutsideCones S X) ↔ (Stable X T ∧ X ≠ S) := by
  constructor
  · rintro ⟨stable, outside⟩
    refine ⟨stable, ?_⟩
    intro same
    subst X
    exact outside.1 (sub_refl S)
  · rintro ⟨stable, different⟩
    exact ⟨stable, other_stable_outside_cones T premise S X stableS stable different⟩

/-- Proper-subset cone exclusion adds nothing to classical candidate filtering
    when the known stable model already has the explicit minimality premise. -/
theorem proper_subset_not_classical_model (T : Theory α)
    (premise : ClassicallyMinimalStable T) (S X : Atoms α)
    (stableS : Stable S T) (proper : ProperSub X S) : ¬ Models X T := by
  intro model
  exact proper.2 ((premise S stableS).2 X proper.1 model)

theorem proper_superset_not_stable (T : Theory α)
    (premise : ClassicallyMinimalStable T) (S X : Atoms α)
    (stableS : Stable S T) (proper : ProperSub S X) : ¬ Stable X T := by
  intro stableX
  exact proper.2 ((premise X stableX).2 S proper.1 stableS.1)

end Zetesis.Feedback
