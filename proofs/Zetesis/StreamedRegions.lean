import Zetesis.StreamedConstraints
import Zetesis.FormulaBounds
import Zetesis.NormalFerraris

/-!
# Original constraint witnesses refute candidate regions

The region and formula readings are those of `Search` and `FormulaBounds`.
For a normalized body, positive and double-negated atoms held in the lower
bound, and default-negated atoms absent from the upper bound, make the body
sure throughout the region. One such original constraint occurrence suffices
to refute the region for any retained Ferraris theory.

The existing bounded source scanner can search for this sufficient witness.
Its completed result means only that this test found no witness: it need not
establish satisfaction, unlike the exact fixed-candidate test in
`StreamedConstraints`. A pending result carries no refutation certificate.
Authenticated source membership and sound region readings are premises; a
complete scan is unnecessary for a witnessed refutation. Complete original
coverage remains required by the final per-candidate constraint checks.

These laws concern original candidate regions, not proper-subset searches under
a frozen reduct. Source lowering, discharged scalar guards, arithmetic-family
admission, catalog coordinates, concrete joins, work limits and cancellation
remain implementation obligations. No Rust coverage-tree construction is assumed.
-/

namespace Zetesis.StreamedRegions

open Ferraris FormulaBounds

universe u v
variable {A : Type u} {ι : Type v}

/-- The normalized body is sure exactly when positive and double-negated atoms
are held and default-negated atoms are cut. This reads original truth, so both
positive signs use the same lower bound; their reduct behavior remains distinct.

Proof outline: a finite conjunction is sure exactly when each conjunct is sure.
The existing implication readings reduce the two negation signs to the stated
lower/upper tests. Ground scalar filtering is already discharged before this body. -/
theorem sure_antecedent_iff (c : Cube A) (r : Semantics.Rule A) :
    Sure c (NormalFerraris.antecedent r) ↔
      (∀ a ∈ r.positive, c.lower a) ∧
      (∀ a ∈ r.gateTrue, c.lower a) ∧
      (∀ a ∈ r.gateFalse, ¬ c.upper a) := by
  have sure_fold (rest : List (Formula A)) (first : Formula A) :
      Sure c (rest.foldl Formula.conj first) ↔
        Sure c first ∧ ∀ F ∈ rest, Sure c F := by
    induction rest generalizing first with
    | nil => simp
    | cons next rest ih =>
      simp only [List.foldl_cons, ih, Sure, FormulaBounds.read, List.mem_cons, forall_eq_or_imp]
      exact and_assoc
  have sure_conjunction (formulas : List (Formula A)) :
      Sure c (NormalFerraris.conjunction formulas) ↔ ∀ F ∈ formulas, Sure c F := by
    cases formulas with
    | nil => simp [NormalFerraris.conjunction, NormalFerraris.truth, Sure, FormulaBounds.read]
    | cons first rest => simp [NormalFerraris.conjunction, sure_fold]
  simp only [NormalFerraris.antecedent, sure_conjunction, NormalFerraris.literals,
    List.forall_mem_append, List.forall_mem_map, Sure, FormulaBounds.read, Ferraris.Neg]
  simp [and_assoc]

/-- Held/cut literals make the complete original body true in every interpretation
between the bounds. `Cube.Contains` states precisely lower ⊆ candidate ⊆ upper;
membership in completed possible support alone is not a lower-bound fact. -/
theorem held_body_satisfied (c : Cube A) (r : Semantics.Rule A)
    (positive : ∀ a ∈ r.positive, c.lower a)
    (doubleNegative : ∀ a ∈ r.gateTrue, c.lower a)
    (negative : ∀ a ∈ r.gateFalse, ¬ c.upper a)
    {M : Atoms A} (inside : c.Contains M) :
    Satisfies M (NormalFerraris.antecedent r) :=
  sure_sound c inside ((sure_antecedent_iff c r).mpr ⟨positive, doubleNegative, negative⟩)

/-- One authenticated original constraint with a sure body refutes the region.
The retained core is arbitrary. The witness can come from any checked prefix;
the rest of the source need not have been traversed to reject this region.

Proof outline: occurrence membership places its negated body in the appended
original theory. The existing sure-body refutation law then excludes every
answer set in the region, already by failure of original satisfaction. -/
theorem sure_occurrence_refutes (P : Theory A) (source : List ι)
    (body : ι → Formula A) (c : Cube A) (occurrence : ι)
    (member : occurrence ∈ source) (sure : Sure c (body occurrence)) :
    ∀ M, c.Contains M → ¬ Stable M (P ++ (source.map body).map Ferraris.Neg) := by
  have original : Ferraris.Neg (body occurrence) ∈ P ++ (source.map body).map Ferraris.Neg :=
    List.mem_append_right _ (List.mem_map.mpr
      ⟨body occurrence, List.mem_map.mpr ⟨occurrence, member, rfl⟩, rfl⟩)
  exact sure_body_refutes _ c original sure

/-- A bounded sufficient-test scan that returns a witness safely refutes the
original candidate region. Only the forward test implication is required;
failure to find a witness need not be complete for region refutation or truth.

Proof outline: `scan_preserves` authenticates the reported occurrence and its
true test. Pointwise test soundness supplies the sure reading; the original
occurrence law excludes every answer set in the region. This is exactly the
soundness premise of `CoverageTree.refuted`, with original stability as `valid`. -/
theorem scan_refutes (P : Theory A) (source : List ι) (body : ι → Formula A)
    (c : Cube A) (test : ι → Bool)
    (sound : ∀ occurrence ∈ source, test occurrence = true → Sure c (body occurrence))
    (fuel : Nat) (occurrence : ι)
    (refuted : StreamedConstraints.scan test fuel source = .violated occurrence) :
    ∀ M, c.Contains M → ¬ Stable M (P ++ (source.map body).map Ferraris.Neg) := by
  have witnessed : occurrence ∈ source ∧ test occurrence = true := by
    simpa only [refuted, StreamedConstraints.Preserves] using
      StreamedConstraints.scan_preserves test fuel source
  exact sure_occurrence_refutes P source body c occurrence witnessed.1
    (sound occurrence witnessed.1 witnessed.2)

/-- A completed sufficient test can leave a violating candidate in the region.
With one undecided atom the held-atom test is false, but the interpretation
containing that atom violates its integrity constraint. Thus `NotRefuted` cannot
replace the final original-satisfaction check, even after an exhaustive scan. -/
theorem complete_scan_can_miss_violation :
    let c : Cube Unit := ⟨Empty, Full⟩
    StreamedConstraints.scan (fun _ : Unit => false) 2 [()] = .complete ∧
      ¬ Sure c (.atom ()) ∧ c.Contains Full ∧
      ¬ Models Full [Ferraris.Neg (.atom ())] := by
  simp [StreamedConstraints.scan, Sure, FormulaBounds.read, Cube.Contains, Sub,
    Empty, Full, Models, Ferraris.Neg, Satisfies]

/-- An interrupted scan can coexist with an original answer set in its region.
The empty interpretation satisfies `:- a` and has no proper subset. Therefore
pending work cannot justify discarding this region as refuted. -/
theorem pending_can_retain_answer :
    let c : Cube Unit := ⟨Empty, Full⟩
    StreamedConstraints.scan (fun _ : Unit => false) 0 [()] = .pending [()] ∧
      c.Contains Empty ∧ Stable Empty [Ferraris.Neg (.atom ())] := by
  simp [StreamedConstraints.scan, Cube.Contains, Sub, Empty, Full,
    Stable, Models, Ferraris.Neg, Satisfies, ProperSub]

end Zetesis.StreamedRegions
