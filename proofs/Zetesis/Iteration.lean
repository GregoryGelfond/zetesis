import Zetesis.Semantics

/-!
Finite derivations and convergence for normalized rule programs.

The intersection definition of `Least` is not assumed to be reached after
finitely many rounds for arbitrary monotone transformers. Here the finite
positive body of each rule makes the union of finite iteration stages closed.
A finite cover of the resulting atoms then gives one stage containing every
derived atom. A finite normalized program supplies such a cover through its
head list, even when the ambient atom type is infinite.

These are semantic existence theorems. They do not establish a round-count
bound, a computable stopping test, backend scheduling fairness, successful
resource allocation, or termination of a source-to-rule materializer.
-/

namespace Zetesis

universe u
variable {α : Type u}

def OmegaIter (T : Transformer α) : Atoms α :=
  fun a => ∃ n, FiniteIter T n a

theorem finiteIter_stage_mono (T : Transformer α) {n m : Nat}
    (hnm : n ≤ m) : Sub (FiniteIter T n) (FiniteIter T m) := by
  obtain ⟨k, rfl⟩ := Nat.exists_eq_add_of_le hnm
  clear hnm
  induction k with
  | zero => exact sub_refl (FiniteIter T n)
  | succ k ih => exact sub_trans ih (finiteIter_grows T (n + k))

/-- Finitely many already-derived atoms all occur together at some stage. -/
theorem finiteList_uniform_stage (T : Transformer α) (xs : List α)
    (hxs : ∀ a, a ∈ xs → OmegaIter T a) :
    ∃ n, ∀ a, a ∈ xs → FiniteIter T n a := by
  induction xs with
  | nil =>
      exact ⟨0, fun _ h => False.elim (List.not_mem_nil h)⟩
  | cons a xs ih =>
      obtain ⟨na, ha⟩ := hxs a (List.mem_cons_self ..)
      obtain ⟨ns, hs⟩ := ih (fun b hb => hxs b (List.mem_cons_of_mem a hb))
      refine ⟨na + ns, ?_⟩
      intro b hb
      cases List.mem_cons.mp hb with
      | inl heq =>
          subst b
          exact finiteIter_stage_mono T (Nat.le_add_right na ns) a ha
      | inr htail =>
          exact finiteIter_stage_mono T (Nat.le_add_left ns na) b (hs b htail)

/-- A finite carrier may contain atoms which never derive. Only the atoms
satisfying `p` need to reach the common stage. -/
theorem finiteList_uniform_stage_if (T : Transformer α) (xs : List α)
    (p : Atoms α) (hp : ∀ a, p a → OmegaIter T a) :
    ∃ n, ∀ a, a ∈ xs → p a → FiniteIter T n a := by
  classical
  induction xs with
  | nil =>
      exact ⟨0, fun _ h => False.elim (List.not_mem_nil h)⟩
  | cons a xs ih =>
      obtain ⟨ns, hs⟩ := ih
      by_cases hpa : p a
      · obtain ⟨na, ha⟩ := hp a hpa
        refine ⟨na + ns, ?_⟩
        intro b hb hpb
        cases List.mem_cons.mp hb with
        | inl heq =>
            subst b
            exact finiteIter_stage_mono T (Nat.le_add_right na ns) a ha
        | inr htail =>
            exact finiteIter_stage_mono T (Nat.le_add_left ns na) b (hs b htail hpb)
      · refine ⟨ns, ?_⟩
        intro b hb hpb
        cases List.mem_cons.mp hb with
        | inl heq => exact False.elim (hpa (heq ▸ hpb))
        | inr htail => exact hs b htail hpb

namespace Semantics

/-- The finite-body property is the key step: one rule cannot require atoms
spread across infinitely many stages without a common finite stage. -/
theorem omegaIter_consequence_closed (P : Program α) (z : Atoms α) :
    Closed (Consequence P z) (OmegaIter (Consequence P z)) := by
  intro a ha
  obtain ⟨r, hr, hh, hf, hg, hb⟩ := ha
  obtain ⟨n, hn⟩ := finiteList_uniform_stage (Consequence P z) r.positive hb
  exact ⟨n + 1, Or.inr ⟨r, hr, hh, hf, hg, hn⟩⟩

theorem gamma_iff_finiteIter (P : Program α) (z : Atoms α) (a : α) :
    Gamma P z a ↔ ∃ n, FiniteIter (Consequence P z) n a := by
  constructor
  · exact gamma_le (omegaIter_consequence_closed P z) a
  · rintro ⟨n, hn⟩
    exact finiteIter_sound (consequence_mono P z) n a hn

theorem gamma_eq_omegaIter (P : Program α) (z : Atoms α) :
    Gamma P z = OmegaIter (Consequence P z) :=
  atoms_ext (gamma_iff_finiteIter P z)

/-- Finite convergence from any finite superset of the derived atoms.
No assumption says that every atom in the cover is derivable. -/
theorem finite_convergence_of_cover (P : Program α) (z : Atoms α)
    (cover : List α) (hcover : ∀ a, Gamma P z a → a ∈ cover) :
    ∃ n, FiniteIter (Consequence P z) n = Gamma P z := by
  obtain ⟨n, hn⟩ := finiteList_uniform_stage_if (Consequence P z) cover
    (Gamma P z) (fun a ha => (gamma_iff_finiteIter P z a).mp ha)
  refine ⟨n, sub_antisymm (finiteIter_sound (consequence_mono P z) n) ?_⟩
  intro a ha
  exact hn a (hcover a ha) ha

def ProgramHeads (P : Program α) : List α := P.filterMap Rule.head

theorem gamma_covered_by_programHeads (P : Program α) (z : Atoms α) :
    ∀ a, Gamma P z a → a ∈ ProgramHeads P := by
  apply gamma_le
  intro a ha
  obtain ⟨r, hr, hh, _, _, _⟩ := ha
  exact List.mem_filterMap.mpr ⟨r, hr, hh⟩

/-- The rule list is finite, so the ambient atom type need not be finite. -/
theorem finite_normalized_program_converges (P : Program α) (z : Atoms α) :
    ∃ n, FiniteIter (Consequence P z) n = Gamma P z :=
  finite_convergence_of_cover P z (ProgramHeads P) (gamma_covered_by_programHeads P z)

end Semantics

end Zetesis
