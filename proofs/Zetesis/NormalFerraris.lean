import Zetesis.Semantics
import Zetesis.RuleFactorization

/-!
# Normalized rules as Ferraris formulas

A normalized rule becomes an implication. Positive body atoms remain atoms;
true gates become double negations; false gates become negations. A missing
head becomes falsum. Body conjunctions follow the left-associated order of
`zetesis-ferraris::normal::from_ground_program`, with truth for an empty body.

The mathematical source has a propositional ground filter. `translate` retains
exactly the rules whose filters hold. `Applicable` describes the already-filtered
input boundary of the Rust operation, which has no filter field. Under that
hypothesis, translation is simply a map of the rule encoding.

Both semantics use the same atom type. In particular, the correspondence applies
to a finite dense carrier including unused atoms; no hypothesis that every atom
occurs in a rule is needed. Frozen-subset correspondence explicitly requires
`J ⊆ M`. This is a semantic translation proof, not a refinement proof for source
grounding, dense-ID conversion, DAG allocation, resource failures or shaders.
-/

namespace Zetesis.NormalFerraris

universe u
variable {α : Type u}
open Ferraris

/-- Truth has the same implication-to-falsum encoding as the ground converter. -/
def truth : Formula α := .imp .bot .bot

/-- The first conjunct needs no identity node; subsequent conjuncts append left. -/
def conjunction : List (Formula α) → Formula α
  | [] => truth
  | first :: rest => rest.foldl Formula.conj first

def literals (r : Semantics.Rule α) : List (Formula α) :=
  r.positive.map Formula.atom ++
    r.gateTrue.map (fun a => Neg (Neg (.atom a))) ++
    r.gateFalse.map (fun a => Neg (.atom a))

def antecedent (r : Semantics.Rule α) : Formula α := conjunction (literals r)

def head : Option α → Formula α
  | none => .bot
  | some a => .atom a

def rule (r : Semantics.Rule α) : Formula α := .imp (antecedent r) (head r.head)

/-- A constraint has no satisfiable head. -/
def HeadHolds (X : Atoms α) : Option α → Prop
  | none => False
  | some a => X a

/-- Ground filtering has already discharged every retained rule's filter. -/
def Applicable (P : Semantics.Program α) : Prop := ∀ r ∈ P, r.filter

open Classical in
/-- Denotational filter selection; no executable evaluator of arbitrary Prop is claimed. -/
noncomputable def translate (P : Semantics.Program α) : Theory α :=
  P.flatMap (fun r => if r.filter then [rule r] else [])

/-- A left fold is true exactly when its initial formula and every appended
    conjunct are true. Repeated formulas and an empty tail need no special premise. -/
theorem satisfies_fold (M : Atoms α) (rest : List (Formula α)) (first : Formula α) :
    Satisfies M (rest.foldl Formula.conj first) ↔
      Satisfies M first ∧ ∀ F ∈ rest, Satisfies M F := by
  induction rest generalizing first with
  | nil => simp
  | cons next rest ih =>
    simp only [List.foldl_cons, ih, Satisfies, List.mem_cons, forall_eq_or_imp]
    exact and_assoc

/-- Frozen conjunction distributes over the same left fold for arbitrary M and J. -/
theorem reduct_fold (M J : Atoms α) (rest : List (Formula α)) (first : Formula α) :
    Satisfies J (Reduct M (rest.foldl Formula.conj first)) ↔
      Satisfies J (Reduct M first) ∧ ∀ F ∈ rest, Satisfies J (Reduct M F) := by
  induction rest generalizing first with
  | nil => simp
  | cons next rest ih =>
    simp only [List.foldl_cons, ih, RuleFactorization.reduct_conj,
      List.mem_cons, forall_eq_or_imp]
    exact and_assoc

/-- The empty-body identity extends finite conjunction truth to every list. -/
theorem satisfies_conjunction (M : Atoms α) (formulas : List (Formula α)) :
    Satisfies M (conjunction formulas) ↔ ∀ F ∈ formulas, Satisfies M F := by
  cases formulas with
  | nil => simp [conjunction, truth, Satisfies]
  | cons first rest => simp [conjunction, satisfies_fold]

/-- Every frozen finite conjunction has exactly its conjuncts' model obligations. -/
theorem reduct_conjunction (M J : Atoms α) (formulas : List (Formula α)) :
    Satisfies J (Reduct M (conjunction formulas)) ↔
      ∀ F ∈ formulas, Satisfies J (Reduct M F) := by
  cases formulas with
  | nil => simp [conjunction, truth, Reduct, Satisfies]
  | cons first rest => simp [conjunction, reduct_fold]

/-- The original body evaluates positive atoms and both gate polarities at M. -/
theorem satisfies_antecedent (r : Semantics.Rule α) (M : Atoms α) :
    Satisfies M (antecedent r) ↔ Semantics.Body r M ∧ Semantics.Gate r M := by
  simp only [antecedent, satisfies_conjunction, literals, List.forall_mem_append,
    List.forall_mem_map, double_neg_satisfies]
  simp only [Ferraris.Neg, Satisfies, Semantics.Body, Semantics.Gate, and_assoc]

/-- A frozen body reads positive atoms at J and both gate polarities at M.
    Positive atom reducts simplify to J precisely under the subset hypothesis. -/
theorem reduct_antecedent (r : Semantics.Rule α) (M J : Atoms α) (hsub : Sub J M) :
    Satisfies J (Reduct M (antecedent r)) ↔
      Semantics.Body r J ∧ Semantics.Gate r M := by
  have atom_at_subset : ∀ a, Satisfies J (Reduct M (.atom a)) ↔ J a := by
    intro a
    rw [atom_reduct]
    exact ⟨And.right, fun present => ⟨hsub a present, present⟩⟩
  simp only [antecedent, reduct_conjunction, literals, List.forall_mem_append,
    List.forall_mem_map, atom_at_subset,
    double_neg_atom_reduct, neg_atom_reduct, Semantics.Body, Semantics.Gate, and_assoc]

/-- An atomic head asserts its atom; a constraint's falsum head is never true. -/
theorem satisfies_head (h : Option α) (M : Atoms α) :
    Satisfies M (head h) ↔ HeadHolds M h := by
  cases h <;> rfl

/-- On a subset of the candidate, reducing an optional head preserves its truth. -/
theorem reduct_head (h : Option α) (M J : Atoms α) (hsub : Sub J M) :
    Satisfies J (Reduct M (head h)) ↔ HeadHolds J h := by
  cases h with
  | none => rfl
  | some a =>
    change Satisfies J (Reduct M (.atom a)) ↔ J a
    rw [atom_reduct]
    exact ⟨And.right, fun present => ⟨hsub a present, present⟩⟩

/-- Original rule satisfaction is the normalized rule implication at M. -/
theorem satisfies_rule (r : Semantics.Rule α) (M : Atoms α) :
    Satisfies M (rule r) ↔
      (Semantics.Body r M → Semantics.Gate r M → HeadHolds M r.head) := by
  simp only [rule, Satisfies, satisfies_antecedent, satisfies_head]
  exact ⟨fun satisfies body gate => satisfies ⟨body, gate⟩,
    fun satisfies ⟨body, gate⟩ => satisfies body gate⟩

/-- A Ferraris rule reduct retains its original M-truth, then imposes the
    normalized frozen implication on J. The original-truth conjunct is essential. -/
theorem reduct_rule (r : Semantics.Rule α) (M J : Atoms α) (hsub : Sub J M) :
    Satisfies J (Reduct M (rule r)) ↔
      Satisfies M (rule r) ∧
        (Semantics.Body r J → Semantics.Gate r M → HeadHolds J r.head) := by
  rw [rule, RuleFactorization.reduct_imp, reduct_antecedent r M J hsub,
    reduct_head r.head M J hsub]
  exact and_congr Iff.rfl
    ⟨fun satisfies body gate => satisfies ⟨body, gate⟩,
      fun satisfies ⟨body, gate⟩ => satisfies body gate⟩

/-- Closure and constraints together say that every applicable gated rule's
    positive body entails its head. This includes the headless case explicitly. -/
theorem reduct_model_iff_rules (P : Semantics.Program α) (M J : Atoms α) :
    Semantics.ReductModel P M J ↔
      ∀ r ∈ P, r.filter → Semantics.Body r J → Semantics.Gate r M → HeadHolds J r.head := by
  constructor
  · intro model r member filter body gate
    cases head_case : r.head with
    | none => exact model.2 r member head_case filter gate body
    | some a =>
      exact model.1 a ⟨r, member, head_case, filter, gate, body⟩
  · intro rules
    have closure : Closed (Semantics.Consequence P M) J := by
      intro a consequence
      obtain ⟨r, member, head_case, filter, gate, body⟩ := consequence
      simpa only [head_case, HeadHolds] using rules r member filter body gate
    have constraints : Semantics.ConstraintsOK P M J := by
      intro r member head_case filter gate body
      simpa only [head_case, HeadHolds] using rules r member filter body gate
    exact ⟨closure, constraints⟩

/-- A translated root comes from a source rule whose ground filter holds. -/
theorem mem_translate (P : Semantics.Program α) (F : Formula α) :
    F ∈ translate P ↔ ∃ r ∈ P, r.filter ∧ rule r = F := by
  classical
  simp only [translate, List.mem_flatMap]
  constructor
  · rintro ⟨r, member, root⟩
    by_cases filter : r.filter
    · exact ⟨r, member, filter, (List.mem_singleton.mp (by simpa [filter] using root)).symm⟩
    · simp [filter] at root
  · rintro ⟨r, member, filter, rfl⟩
    exact ⟨r, member, by simp [filter]⟩

/-- A false ground filter contributes no formula root, even if its unfiltered
    rule would assert an atom or an unconditional constraint. -/
theorem false_filter_omitted (r : Semantics.Rule α) (filter : ¬ r.filter) :
    translate [r] = [] := by
  classical
  simp [translate, filter]

/-- On the already-filtered ground boundary, translation has exactly one root
    per rule in the input order, as in `from_ground_program`. -/
theorem translate_applicable (P : Semantics.Program α) (applicable : Applicable P) :
    translate P = P.map rule := by
  classical
  induction P with
  | nil => rfl
  | cons r rest ih =>
    have filter : r.filter := applicable r (by simp)
    have rest_applicable : Applicable rest :=
      fun other member => applicable other (List.mem_cons_of_mem r member)
    simpa only [translate, List.flatMap_cons, if_pos filter, List.singleton_append,
      List.map_cons] using congrArg (List.cons (rule r)) (ih rest_applicable)

/-- Original models of the translated theory coincide with normalized models
    of their own frozen rules, including all applicable constraints. -/
theorem models_translate (P : Semantics.Program α) (M : Atoms α) :
    Models M (translate P) ↔ Semantics.ReductModel P M M := by
  rw [reduct_model_iff_rules]
  constructor
  · intro models r member filter
    exact (satisfies_rule r M).mp
      (models (rule r) ((mem_translate P (rule r)).mpr ⟨r, member, filter, rfl⟩))
  · intro rules F member
    obtain ⟨r, rule_member, filter, rfl⟩ := (mem_translate P F).mp member
    exact (satisfies_rule r M).mpr (rules r rule_member filter)

/-- Exact frozen-subset correspondence, including rejection of an M that does
    not satisfy its own normalized reduct. No candidate-model assumption is hidden. -/
theorem models_frozen_translate (P : Semantics.Program α) (M J : Atoms α)
    (hsub : Sub J M) :
    Models J (ReductTheory M (translate P)) ↔
      Semantics.ReductModel P M M ∧ Semantics.ReductModel P M J := by
  have per_rule : Models J (ReductTheory M (translate P)) ↔
      ∀ r ∈ P, r.filter → Satisfies J (Reduct M (rule r)) := by
    constructor
    · intro models r member filter
      exact models _ (List.mem_map.mpr
        ⟨rule r, (mem_translate P _).mpr ⟨r, member, filter, rfl⟩, rfl⟩)
    · intro rules F member
      obtain ⟨original, original_member, rfl⟩ := List.mem_map.mp member
      obtain ⟨r, rule_member, filter, rfl⟩ := (mem_translate P original).mp original_member
      exact rules r rule_member filter
  rw [per_rule, reduct_model_iff_rules, reduct_model_iff_rules]
  constructor
  · intro rules
    have original : ∀ r ∈ P, r.filter →
        Semantics.Body r M → Semantics.Gate r M → HeadHolds M r.head := by
      intro r member filter
      exact (satisfies_rule r M).mp ((reduct_rule r M J hsub).mp (rules r member filter)).1
    have frozen : ∀ r ∈ P, r.filter →
        Semantics.Body r J → Semantics.Gate r M → HeadHolds J r.head := by
      intro r member filter
      exact ((reduct_rule r M J hsub).mp (rules r member filter)).2
    exact ⟨original, frozen⟩
  · intro ⟨original, frozen⟩ r member filter
    exact (reduct_rule r M J hsub).mpr
      ⟨(satisfies_rule r M).mpr (original r member filter), frozen r member filter⟩

/-- For a candidate that satisfies its normalized reduct, every subset has
    exactly the same model verdict under both frozen reduct representations. -/
theorem frozen_subset_model_iff (P : Semantics.Program α) (M J : Atoms α)
    (candidate : Semantics.ReductModel P M M) (hsub : Sub J M) :
    Models J (ReductTheory M (translate P)) ↔ Semantics.ReductModel P M J := by
  rw [models_frozen_translate P M J hsub]
  exact ⟨And.right, fun model => ⟨candidate, model⟩⟩

/-- An answer set of normalized reduct semantics is exactly an answer set of
    its Ferraris translation. This relates independently defined minimality
    predicates; neither definition is replaced by an alias for the other. -/
theorem answer_set_iff (P : Semantics.Program α) (M : Atoms α) :
    Semantics.Stable P M ↔ Ferraris.Stable M (translate P) := by
  rw [Ferraris.stable_iff_minimal_reduct]
  constructor
  · intro ⟨candidate, minimal⟩
    have original : Models M (ReductTheory M (translate P)) :=
      (models_frozen_translate P M M (sub_refl M)).mpr ⟨candidate, candidate⟩
    have minimality : ∀ J, Sub J M →
        Models J (ReductTheory M (translate P)) → Sub M J := by
      intro J subset model
      exact minimal J ((frozen_subset_model_iff P M J candidate subset).mp model) subset
    exact ⟨original, minimality⟩
  · intro ⟨original, minimal⟩
    have candidate : Semantics.ReductModel P M M :=
      ((models_frozen_translate P M M (sub_refl M)).mp original).1
    have minimality : ∀ J, Semantics.ReductModel P M J → Sub J M → Sub M J := by
      intro J model subset
      exact minimal J subset ((frozen_subset_model_iff P M J candidate subset).mpr model)
    exact ⟨candidate, minimality⟩

/-- The direct rule map used after ground filtering preserves answer sets. -/
theorem applicable_answer_set_iff (P : Semantics.Program α) (M : Atoms α)
    (applicable : Applicable P) :
    Semantics.Stable P M ↔ Ferraris.Stable M (P.map rule) := by
  rw [← translate_applicable P applicable]
  exact answer_set_iff P M

/-- The direct ground-rule map preserves every subset's frozen model verdict
    when the candidate is a model and all retained filters have been discharged. -/
theorem applicable_frozen_subset_model_iff (P : Semantics.Program α) (M J : Atoms α)
    (applicable : Applicable P) (candidate : Semantics.ReductModel P M M)
    (hsub : Sub J M) :
    Models J (ReductTheory M (P.map rule)) ↔ Semantics.ReductModel P M J := by
  rw [← translate_applicable P applicable]
  exact frozen_subset_model_iff P M J candidate hsub

/-- Least normalized closure plus constraints is an exact answer-set test for
    this Ferraris fragment, not an alternative acceptance semantics. -/
theorem ferraris_answer_set_iff_closure (P : Semantics.Program α) (M : Atoms α) :
    Ferraris.Stable M (translate P) ↔
      Semantics.Gamma P M = M ∧ Semantics.ConstraintsOK P M M := by
  rw [← answer_set_iff]
  exact Semantics.stable_iff_gamma P M

end Zetesis.NormalFerraris
