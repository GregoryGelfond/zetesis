import Zetesis.AxisFolds
import Zetesis.LearnedRegions
import Zetesis.ThresholdReduct
import Zetesis.PartitionCapacities
import Zetesis.FormulaChains

/-!
# Rows of a consequence class over an algebra

Candidate generation may narrow a region by what its class of interpretations
entails. Here the consequence is algebraic. A term pairs an atom with a value,
and a list of terms has a measure in each interpretation: the fold, under a
commutative monoid (`AxisFolds.Reduction`), of the values of the terms whose
atoms hold. Two shapes of consequence share the measure.

- **Equations in an abelian group.** A measure equals a constant. Parity is the
  group of Booleans under exclusive-or (`parity`); counts modulo a number would
  be another group. Equations of a class add (`equation_sum`), so the
  combinations an elimination finds are equations of the same class, and an
  atom's terms with inverse values cancel (`measure_cancel`). Equal measures
  transport equations (`equation_of_agreement`); in parity, retaining exactly
  the odd-multiplicity atoms preserves the measure (`parity_measure_odd`), so a
  pointwise XOR of finite masks adds their equations (`parity_equation_of_xor`).
  Given every other atom of an equation, the equation isolates one atom's contribution
  (`equation_isolates`): it forces the atom (`equation_forces_true`,
  `equation_forces_false`) or excludes the conditions given
  (`equation_excludes`).
- **Bounds in an ordered commutative monoid.** A measure lies above or below a
  bound. Counts are the natural numbers under addition (`counting`); weighted
  sums would be another instance. Two families of rows covering the same terms
  measure them twice, so lower bounds that the upper bounds cannot reach exclude
  every member (`double_count_excludes`). In a region, held atoms raise the
  lower bounds and atoms not yet cut cap the measures
  (`double_count_excludes_region`), and a cut certificate excludes a region of
  a bipartite system (`cut_excludes_region`).

Every law is stated for an arbitrary consequence class C, a predicate on
interpretations, as `LearnedRegions.RowOf` is. A forced value is a row of the
class whose conditions are the given literals, so `LearnedRegions`'
`consequence_row_forces` carries it into a region, and conditions that exclude
the class exclude a region deciding them (`excludes_region`). For a theory's
answer sets the class is `fun M => Stable M T`. A membership query's
countermodels are another class, with the same laws.

A row enters a class through its provenance, checked once against its source:
- a property local to a few atoms holds in every model of a group of formulas
  over them when every pattern of those atoms that violates it is refuted by a
  formula of the group (`entailed_by_patterns`, which rests on
  `Ferraris.satisfies_congr`); every answer set models the theory and so the
  group (`entailed_by_part`);
- a property local to an atom's completion holds in every answer set when every
  pattern satisfying the completion has it (`entailed_by_completion`, from
  `DisjunctiveSupport.answer_set_completion`);
- a threshold over atom conditions bounds its terms' count
  (`lower_of_threshold`, `upper_of_negated_threshold`, from
  `ThresholdReduct.original`), and a guard releases a guarded threshold where it
  holds (`guard_releases`); a clause forbidding every listed atom from being
  false gives a lower bound of one (`lower_of_clause`).

Not proved here: which rows, groups, completions, thresholds and guards a
recognizer accepts; elimination, flow, or any search that finds a combination
or a certificate; coordinate assignment, bit packing and word operations;
resource accounting; and the narrower's use of a consequence. A probe of an atom
is `Region.split_partition` with its held half excluded.
-/

namespace Zetesis.AlgebraicRows

open Ferraris AxisFolds LearnedRegions

universe u v w
variable {α : Type u} {V : Type v}

/-! ## Measures -/

/-- A term: an atom, and the value it contributes when the atom holds. -/
abbrev Term (α : Type u) (V : Type v) := α × V

open Classical in
/-- A term's contribution in an interpretation: its value when its atom holds,
and the identity otherwise. -/
noncomputable def contribution (r : Reduction V) (M : Atoms α) (t : Term α V) : V :=
  if M t.1 then t.2 else r.unit

theorem contribution_of_holds (r : Reduction V) {M : Atoms α} {t : Term α V}
    (holds : M t.1) : contribution r M t = t.2 := by
  simp [contribution, holds]

theorem contribution_of_fails (r : Reduction V) {M : Atoms α} {t : Term α V}
    (fails : ¬ M t.1) : contribution r M t = r.unit := by
  simp [contribution, fails]

/-- The measure of a list of terms in an interpretation: the fold of the terms'
contributions. Lists count occurrences. -/
noncomputable def measure (r : Reduction V) (M : Atoms α) (terms : List (Term α V)) : V :=
  r.fold (contribution r M) terms

theorem measure_cons (r : Reduction V) (M : Atoms α) (t : Term α V)
    (rest : List (Term α V)) :
    measure r M (t :: rest) = r.op (contribution r M t) (measure r M rest) :=
  r.fold_cons _ t rest

/-- Measures add over concatenation (`Reduction.fold_append`). -/
theorem measure_append (r : Reduction V) (M : Atoms α) (first second : List (Term α V)) :
    measure r M (first ++ second) = r.op (measure r M first) (measure r M second) :=
  r.fold_append _ first second

/-- A measure does not depend on the order of the terms (`Reduction.fold_perm`). -/
theorem measure_perm (r : Reduction V) (M : Atoms α) {first second : List (Term α V)}
    (same : first.Perm second) : measure r M first = measure r M second :=
  r.fold_perm _ same

/-- The measure of the flattened parts is the fold of the parts' measures. -/
theorem measure_flatten (r : Reduction V) (M : Atoms α) (parts : List (List (Term α V))) :
    measure r M parts.flatten = r.fold (measure r M) parts := by
  induction parts with
  | nil => rfl
  | cons part rest ih => rw [List.flatten_cons, measure_append, ih, r.fold_cons]

/-- **Locality.** A measure depends only on the truth of its terms' atoms:
interpretations that agree on a set containing them measure the terms alike. -/
theorem measure_congr (r : Reduction V) {S M N : Atoms α} (agree : EqOn S M N)
    (terms : List (Term α V)) (inside : ∀ t ∈ terms, S t.1) :
    measure r M terms = measure r N terms := by
  apply r.fold_congr
  intro t member
  have same : M t.1 ↔ N t.1 := agree t.1 (inside t member)
  by_cases holds : M t.1
  · rw [contribution_of_holds r holds, contribution_of_holds r (same.mp holds)]
  · rw [contribution_of_fails r holds,
      contribution_of_fails r (fun present => holds (same.mpr present))]

/-- The terms of one atom. -/
def own [DecidableEq α] (x : α) (terms : List (Term α V)) : List (Term α V) :=
  terms.filter (fun t => decide (t.1 = x))

/-- The terms of every other atom. -/
def others [DecidableEq α] (x : α) (terms : List (Term α V)) : List (Term α V) :=
  terms.filter (fun t => !decide (t.1 = x))

/-- An atom's coefficient: the fold of its own terms' values. -/
def coefficient (r : Reduction V) [DecidableEq α] (x : α) (terms : List (Term α V)) : V :=
  r.fold Prod.snd (own x terms)

/-- A measure is the atom's own terms' measure combined with the others'. -/
theorem measure_split (r : Reduction V) [DecidableEq α] (M : Atoms α) (x : α)
    (terms : List (Term α V)) :
    measure r M terms = r.op (measure r M (own x terms)) (measure r M (others x terms)) := by
  rw [← measure_append]
  exact (measure_perm r M (List.filter_append_perm _ terms)).symm

/-- Terms of a single atom measure as that atom's contribution with the fold of
their values. -/
theorem measure_single (r : Reduction V) (M : Atoms α) (x : α) (terms : List (Term α V))
    (single : ∀ t ∈ terms, t.1 = x) :
    measure r M terms = contribution r M (x, r.fold Prod.snd terms) := by
  by_cases holds : M x
  · have each : ∀ t ∈ terms, contribution r M t = t.2 := by
      intro t member
      exact contribution_of_holds r (by rw [single t member]; exact holds)
    rw [contribution_of_holds r (t := (x, r.fold Prod.snd terms)) holds]
    exact r.fold_congr each
  · have each : ∀ t ∈ terms, contribution r M t = r.unit := by
      intro t member
      exact contribution_of_fails r (by rw [single t member]; exact holds)
    rw [contribution_of_fails r (t := (x, r.fold Prod.snd terms)) holds]
    exact (r.fold_congr each).trans (r.fold_unit terms)

/-- An atom's own terms measure as its contribution with its coefficient. -/
theorem measure_own (r : Reduction V) [DecidableEq α] (M : Atoms α) (x : α)
    (terms : List (Term α V)) :
    measure r M (own x terms) = contribution r M (x, coefficient r x terms) :=
  measure_single r M x (own x terms)
    (fun _ member => of_decide_eq_true (List.mem_filter.mp member).2)

/-! ## Equations in an abelian group -/

/-- An abelian group: a commutative monoid whose every value has an inverse. -/
structure AbelianGroup (V : Type v) extends Reduction V where
  neg : V → V
  op_neg : ∀ a, op a (neg a) = unit

/-- In a group, a sum determines either summand from the other. -/
theorem AbelianGroup.solve (g : AbelianGroup V) {a b c : V} (sum : g.op a b = c) :
    a = g.op c (g.neg b) := by
  calc a = g.op a g.unit := (g.toReduction.op_unit a).symm
    _ = g.op a (g.op b (g.neg b)) := by rw [g.op_neg]
    _ = g.op (g.op a b) (g.neg b) := (g.assoc a b (g.neg b)).symm
    _ = g.op c (g.neg b) := by rw [sum]

/-- An atom's terms with inverse values cancel. -/
theorem measure_cancel (g : AbelianGroup V) (M : Atoms α) (x : α) (value : V) :
    measure g.toReduction M [(x, value), (x, g.neg value)] = g.unit := by
  rw [measure_cons, measure_cons]
  by_cases holds : M x
  · rw [contribution_of_holds g.toReduction (t := (x, value)) holds,
      contribution_of_holds g.toReduction (t := (x, g.neg value)) holds]
    show g.op value (g.op (g.neg value) g.unit) = g.unit
    rw [g.toReduction.op_unit, g.op_neg]
  · rw [contribution_of_fails g.toReduction (t := (x, value)) holds,
      contribution_of_fails g.toReduction (t := (x, g.neg value)) holds]
    show g.op g.unit (g.op g.unit g.unit) = g.unit
    rw [g.unit_op, g.unit_op]

/-- An equation: terms whose measure must equal a constant. -/
structure Equation (α : Type u) (V : Type v) where
  terms : List (Term α V)
  constant : V

namespace Equation

/-- An interpretation satisfies an equation when the terms' measure is the
constant. -/
def Holds (r : Reduction V) (M : Atoms α) (e : Equation α V) : Prop :=
  measure r M e.terms = e.constant

/-- The sum of two equations: their terms together, and their constants combined. -/
def add (r : Reduction V) (e f : Equation α V) : Equation α V :=
  ⟨e.terms ++ f.terms, r.op e.constant f.constant⟩

/-- The empty equation. -/
def zero (r : Reduction V) : Equation α V := ⟨[], r.unit⟩

/-- The sum of a list of equations. -/
def sum (r : Reduction V) (es : List (Equation α V)) : Equation α V :=
  es.foldr (add r) (zero r)

end Equation

/-- An equation of a consequence class: every member of the class satisfies it.
The class names what the equation is a consequence of. -/
def EquationOf (r : Reduction V) (C : Atoms α → Prop) (e : Equation α V) : Prop :=
  ∀ M, C M → e.Holds r M

/-- Replacing an equation's terms by terms with the same measure in every member
of its class preserves the equation, provided the constant is unchanged. This
transports a consequence between representations without rechecking its source. -/
theorem equation_of_agreement (r : Reduction V) (C : Atoms α → Prop)
    {e f : Equation α V} (of : EquationOf r C e)
    (agree : ∀ M, C M → measure r M f.terms = measure r M e.terms)
    (constant : f.constant = e.constant) : EquationOf r C f := by
  intro M member
  exact (agree M member).trans ((of M member).trans constant.symm)

/-- The sum of two equations of a class is an equation of the class. -/
theorem equation_add (r : Reduction V) (C : Atoms α → Prop) {e f : Equation α V}
    (left : EquationOf r C e) (right : EquationOf r C f) :
    EquationOf r C (e.add r f) := by
  intro M member
  show measure r M (e.terms ++ f.terms) = r.op e.constant f.constant
  rw [measure_append, left M member, right M member]

/-- **Combination.** The sum of equations of a class is an equation of the
class: an elimination's combinations, with repetition, are consequences of the
class it starts from.

Proof. By induction on the list; the empty sum's measure is the identity, and
`equation_add` adds each further equation. -/
theorem equation_sum (r : Reduction V) (C : Atoms α → Prop) (es : List (Equation α V))
    (each : ∀ e ∈ es, EquationOf r C e) : EquationOf r C (Equation.sum r es) := by
  induction es with
  | nil => exact fun _ _ => rfl
  | cons e rest ih =>
    exact equation_add r C (each e (List.mem_cons_self ..))
      (ih (fun f member => each f (List.mem_cons_of_mem e member)))

/-- An equation of a class is an equation of every narrower class: an equation of
the models of some of a theory's formulas holds for the theory's answer sets. -/
theorem equation_of_narrower_class (r : Reduction V) (C D : Atoms α → Prop)
    (inclusion : ∀ M, D M → C M) (e : Equation α V) (of : EquationOf r C e) :
    EquationOf r D e :=
  fun M member => of M (inclusion M member)

open Classical in
/-- An atom's truth value in an interpretation, as a literal's sign. -/
noncomputable def truth (M : Atoms α) (a : α) : Bool := decide (M a)

/-- An interpretation satisfies the literal giving an atom its own truth value. -/
theorem holds_truth (M : Atoms α) (a : α) : Holds M (a, truth M a) := by
  by_cases present : M a
  · have sign : truth M a = true := by simp [truth, present]
    rw [sign]
    exact present
  · have sign : truth M a = false := by simp [truth, present]
    rw [sign]
    exact present

/-- A region decides the literal giving an atom its held value when it holds or
has cut the atom. -/
theorem decides_truth (c : Region α) (a : α) (decided : c.lower a ∨ ¬ c.upper a) :
    Decides c (a, truth c.lower a) := by
  by_cases held : c.lower a
  · have sign : truth c.lower a = true := by simp [truth, held]
    rw [sign]
    exact held
  · have sign : truth c.lower a = false := by simp [truth, held]
    rw [sign]
    exact decided.resolve_left held

/-- Literals giving the terms' atoms their values in σ make every interpretation
satisfying them measure the terms as σ does. -/
theorem measure_of_given (r : Reduction V) {M σ : Atoms α} {B : List (Literal α)}
    {terms : List (Term α V)} (given : ∀ t ∈ terms, (t.1, truth σ t.1) ∈ B)
    (satisfied : ∀ l ∈ B, Holds M l) : measure r M terms = measure r σ terms := by
  have agree : EqOn (fun a => ∃ t ∈ terms, t.1 = a) M σ := by
    rintro a ⟨t, member, rfl⟩
    have literal : Holds M (t.1, truth σ t.1) := satisfied _ (given t member)
    by_cases present : σ t.1
    · have sign : truth σ t.1 = true := by simp [truth, present]
      rw [sign] at literal
      exact ⟨fun _ => present, fun _ => literal⟩
    · have sign : truth σ t.1 = false := by simp [truth, present]
      rw [sign] at literal
      exact ⟨fun holds => absurd holds literal, fun holds => absurd holds present⟩
  exact measure_congr r agree terms (fun t member => ⟨t, member, rfl⟩)

/-- **Isolation.** In every member of the class satisfying literals that give the
other atoms of an equation their values in σ, one atom's contribution, with its
coefficient, is the constant minus the others' measure in σ.

Proof. The member satisfies the equation. Its measure is the atom's own terms'
measure, which is the atom's contribution with its coefficient
(`measure_own`), combined with the others', which the literals fix at their
measure in σ (`measure_of_given`). The group solves for the contribution. -/
theorem equation_isolates [DecidableEq α] (g : AbelianGroup V) (C : Atoms α → Prop)
    (e : Equation α V) (of : EquationOf g.toReduction C e) (x : α) (σ : Atoms α)
    (B : List (Literal α)) (given : ∀ t ∈ others x e.terms, (t.1, truth σ t.1) ∈ B) :
    ∀ M, C M → (∀ l ∈ B, Holds M l) →
      contribution g.toReduction M (x, coefficient g.toReduction x e.terms) =
        g.op e.constant (g.neg (measure g.toReduction σ (others x e.terms))) := by
  intro M member satisfied
  have holds : measure g.toReduction M e.terms = e.constant := of M member
  have rest : measure g.toReduction M (others x e.terms) =
      measure g.toReduction σ (others x e.terms) :=
    measure_of_given g.toReduction given satisfied
  rw [measure_split g.toReduction M x e.terms, measure_own, rest] at holds
  exact g.solve holds

/-- When the isolated value is the atom's coefficient, and that is not the
identity, the conditions force the atom true: a row of the class. -/
theorem equation_forces_true [DecidableEq α] (g : AbelianGroup V) (C : Atoms α → Prop)
    (e : Equation α V) (of : EquationOf g.toReduction C e) (x : α) (σ : Atoms α)
    (B : List (Literal α)) (given : ∀ t ∈ others x e.terms, (t.1, truth σ t.1) ∈ B)
    (solved : g.op e.constant (g.neg (measure g.toReduction σ (others x e.terms))) =
      coefficient g.toReduction x e.terms)
    (distinct : coefficient g.toReduction x e.terms ≠ g.unit) :
    RowOf C B [(x, true)] := by
  intro M member satisfied o output
  have isolated := equation_isolates g C e of x σ B given M member satisfied
  rw [List.mem_singleton.mp output]
  show M x
  apply Classical.byContradiction
  intro fails
  rw [contribution_of_fails g.toReduction (t := (x, coefficient g.toReduction x e.terms)) fails,
    solved] at isolated
  exact distinct isolated.symm

/-- When the isolated value is the identity, and the atom's coefficient is not,
the conditions force the atom false: a row of the class. -/
theorem equation_forces_false [DecidableEq α] (g : AbelianGroup V) (C : Atoms α → Prop)
    (e : Equation α V) (of : EquationOf g.toReduction C e) (x : α) (σ : Atoms α)
    (B : List (Literal α)) (given : ∀ t ∈ others x e.terms, (t.1, truth σ t.1) ∈ B)
    (solved : g.op e.constant (g.neg (measure g.toReduction σ (others x e.terms))) = g.unit)
    (distinct : coefficient g.toReduction x e.terms ≠ g.unit) :
    RowOf C B [(x, false)] := by
  intro M member satisfied o output
  have isolated := equation_isolates g C e of x σ B given M member satisfied
  rw [List.mem_singleton.mp output]
  show ¬ M x
  intro holds
  rw [contribution_of_holds g.toReduction (t := (x, coefficient g.toReduction x e.terms)) holds,
    solved] at isolated
  exact distinct isolated

/-- Conditions that exclude every member of a class: no member satisfies them
all. -/
def Excludes (C : Atoms α → Prop) (B : List (Literal α)) : Prop :=
  ∀ M, C M → ¬ ∀ l ∈ B, Holds M l

/-- When the isolated value is neither the identity nor the atom's coefficient, no
contribution of the atom can supply it: the conditions exclude the class. -/
theorem equation_excludes [DecidableEq α] (g : AbelianGroup V) (C : Atoms α → Prop)
    (e : Equation α V) (of : EquationOf g.toReduction C e) (x : α) (σ : Atoms α)
    (B : List (Literal α)) (given : ∀ t ∈ others x e.terms, (t.1, truth σ t.1) ∈ B)
    (unmatched : g.op e.constant (g.neg (measure g.toReduction σ (others x e.terms))) ≠ g.unit)
    (unreached : g.op e.constant (g.neg (measure g.toReduction σ (others x e.terms))) ≠
      coefficient g.toReduction x e.terms) :
    Excludes C B := by
  intro M member satisfied
  have isolated := equation_isolates g C e of x σ B given M member satisfied
  by_cases holds : M x
  · rw [contribution_of_holds g.toReduction (t := (x, coefficient g.toReduction x e.terms)) holds]
      at isolated
    exact unreached isolated.symm
  · rw [contribution_of_fails g.toReduction (t := (x, coefficient g.toReduction x e.terms)) holds]
      at isolated
    exact unmatched isolated.symm

/-- Literals giving every atom of an equation a value under which its measure
differs from the constant exclude the class. This needs no group. -/
theorem equation_excludes_given (r : Reduction V) (C : Atoms α → Prop) (e : Equation α V)
    (of : EquationOf r C e) (σ : Atoms α) (B : List (Literal α))
    (given : ∀ t ∈ e.terms, (t.1, truth σ t.1) ∈ B)
    (wrong : measure r σ e.terms ≠ e.constant) : Excludes C B := by
  intro M member satisfied
  apply wrong
  rw [← measure_of_given r given satisfied]
  exact of M member

/-- Conditions that exclude a class exclude every member of it in a region that
decides them. -/
theorem excludes_region (C : Atoms α → Prop) (B : List (Literal α)) (excluded : Excludes C B)
    (c : Region α) (decided : ∀ l ∈ B, Decides c l) : ∀ z, c.Contains z → ¬ C z :=
  fun z inside member => excluded z member (fun l listed => decides_holds inside (decided l listed))

/-! ## Bounds in an ordered commutative monoid -/

/-- An ordered commutative monoid: a commutative monoid with a preorder that its
operation preserves in both arguments. -/
structure OrderedReduction (V : Type v) extends Reduction V where
  le : V → V → Prop
  le_refl : ∀ a, le a a
  le_trans : ∀ a b c, le a b → le b c → le a c
  op_le_op : ∀ a b c d, le a b → le c d → le (op a c) (op b d)

/-- Terms whose values are at least the identity, as counts' weights are. -/
def Positive (o : OrderedReduction V) (terms : List (Term α V)) : Prop :=
  ∀ t ∈ terms, o.le o.unit t.2

/-- A lower bound of a class: every member's measure of the terms is at least
the bound. -/
def LowerOf (o : OrderedReduction V) (C : Atoms α → Prop) (terms : List (Term α V))
    (bound : V) : Prop :=
  ∀ M, C M → o.le bound (measure o.toReduction M terms)

/-- An upper bound of a class: every member's measure of the terms is at most the
bound. -/
def UpperOf (o : OrderedReduction V) (C : Atoms α → Prop) (terms : List (Term α V))
    (bound : V) : Prop :=
  ∀ M, C M → o.le (measure o.toReduction M terms) bound

/-- A lower bound of the class's members in a region. -/
def LowerIn (o : OrderedReduction V) (C : Atoms α → Prop) (c : Region α)
    (terms : List (Term α V)) (bound : V) : Prop :=
  ∀ z, c.Contains z → C z → o.le bound (measure o.toReduction z terms)

/-- An upper bound of the class's members in a region. -/
def UpperIn (o : OrderedReduction V) (C : Atoms α → Prop) (c : Region α)
    (terms : List (Term α V)) (bound : V) : Prop :=
  ∀ z, c.Contains z → C z → o.le (measure o.toReduction z terms) bound

/-- Folds preserve the order pointwise. -/
theorem fold_le_fold (o : OrderedReduction V) {β : Type w} {f g : β → V} (members : List β)
    (each : ∀ m ∈ members, o.le (f m) (g m)) :
    o.le (o.toReduction.fold f members) (o.toReduction.fold g members) := by
  induction members with
  | nil => exact o.le_refl o.unit
  | cons head rest ih =>
    rw [o.toReduction.fold_cons, o.toReduction.fold_cons]
    exact o.op_le_op _ _ _ _ (each head (List.mem_cons_self ..))
      (ih (fun m member => each m (List.mem_cons_of_mem head member)))

/-- A measure of positive terms is at least the identity. -/
theorem unit_le_measure (o : OrderedReduction V) (M : Atoms α) (terms : List (Term α V))
    (positive : Positive o terms) : o.le o.unit (measure o.toReduction M terms) := by
  have units : o.le (o.toReduction.fold (fun _ => o.unit) terms)
      (measure o.toReduction M terms) := by
    apply fold_le_fold o terms
    intro t member
    by_cases holds : M t.1
    · rw [contribution_of_holds o.toReduction holds]
      exact positive t member
    · rw [contribution_of_fails o.toReduction holds]
      exact o.le_refl o.unit
  rwa [o.toReduction.fold_unit] at units

/-- A measure of positive terms grows with the interpretation. -/
theorem measure_mono (o : OrderedReduction V) {M N : Atoms α} (included : Sub M N)
    (terms : List (Term α V)) (positive : Positive o terms) :
    o.le (measure o.toReduction M terms) (measure o.toReduction N terms) := by
  apply fold_le_fold o terms
  intro t member
  by_cases holds : M t.1
  · rw [contribution_of_holds o.toReduction holds,
      contribution_of_holds o.toReduction (included t.1 holds)]
    exact o.le_refl t.2
  · rw [contribution_of_fails o.toReduction holds]
    by_cases later : N t.1
    · rw [contribution_of_holds o.toReduction later]
      exact positive t member
    · rw [contribution_of_fails o.toReduction later]
      exact o.le_refl o.unit

/-- In a region, the held atoms' measure is a lower bound of every member. -/
theorem lower_in_of_held (o : OrderedReduction V) (C : Atoms α → Prop) (c : Region α)
    (terms : List (Term α V)) (positive : Positive o terms) :
    LowerIn o C c terms (measure o.toReduction c.lower terms) :=
  fun _ inside _ => measure_mono o inside.1 terms positive

/-- In a region, the measure of the atoms not cut is an upper bound of every
member. -/
theorem upper_in_of_possible (o : OrderedReduction V) (C : Atoms α → Prop) (c : Region α)
    (terms : List (Term α V)) (positive : Positive o terms) :
    UpperIn o C c terms (measure o.toReduction c.upper terms) :=
  fun _ inside _ => measure_mono o inside.2 terms positive

theorem lower_in_of_class (o : OrderedReduction V) (C : Atoms α → Prop) (c : Region α)
    {terms : List (Term α V)} {bound : V} (lower : LowerOf o C terms bound) :
    LowerIn o C c terms bound :=
  fun z _ member => lower z member

theorem upper_in_of_class (o : OrderedReduction V) (C : Atoms α → Prop) (c : Region α)
    {terms : List (Term α V)} {bound : V} (upper : UpperOf o C terms bound) :
    UpperIn o C c terms bound :=
  fun z _ member => upper z member

/-- The measure of a family's rows together is the fold of the rows' measures. -/
theorem measure_rows (r : Reduction V) (M : Atoms α) (rows : List (List (Term α V) × V)) :
    measure r M (rows.map Prod.fst).flatten = r.fold (fun row => measure r M row.1) rows := by
  rw [measure_flatten, r.fold_map]

/-- **The double count in a region.** Two families of rows each cover the same
terms, the first with lower bounds and the second with upper bounds that hold for
the class's members in a region. If the lower bounds' fold does not lie below the
upper bounds', the region contains no member of the class. Held atoms may raise a
lower bound (`lower_in_of_held`), and atoms not cut may lower an upper bound
(`upper_in_of_possible`).

Proof. Measure the covered terms twice. The first family's bounds lie below their
rows' measures, whose fold is the terms' measure (`measure_rows`, `measure_perm`);
so does the second family's, whose bounds lie above. -/
theorem double_count_excludes_region (o : OrderedReduction V) (C : Atoms α → Prop)
    (c : Region α) (S : List (Term α V)) (first second : List (List (Term α V) × V))
    (firstCovers : (first.map Prod.fst).flatten.Perm S)
    (secondCovers : (second.map Prod.fst).flatten.Perm S)
    (lower : ∀ row ∈ first, LowerIn o C c row.1 row.2)
    (upper : ∀ row ∈ second, UpperIn o C c row.1 row.2)
    (exceeds : ¬ o.le (o.toReduction.fold Prod.snd first) (o.toReduction.fold Prod.snd second)) :
    ∀ z, c.Contains z → ¬ C z := by
  intro z inside member
  have fromBelow : o.le (o.toReduction.fold Prod.snd first) (measure o.toReduction z S) := by
    rw [← measure_perm o.toReduction z firstCovers, measure_rows]
    exact fold_le_fold o first (fun row listed => lower row listed z inside member)
  have fromAbove : o.le (measure o.toReduction z S) (o.toReduction.fold Prod.snd second) := by
    rw [← measure_perm o.toReduction z secondCovers, measure_rows]
    exact fold_le_fold o second (fun row listed => upper row listed z inside member)
  exact exceeds (o.le_trans _ _ _ fromBelow fromAbove)

open Classical in
/-- **A double count with partially refreshed bounds.** Two finite families
cover the same terms. Each old bound holds in an enclosing region and each new
bound holds in the current region. Choose either bound at every occurrence.
If those chosen lower bounds exceed the chosen upper bounds, the current
region has no member of the class.

Proof. Every member of the current region is also in the enclosing region,
so both choices of bound hold. Attach the chosen bounds to their unchanged
terms and apply `double_count_excludes_region`.

The selectors describe occurrences, not necessarily distinct source rows.
The two families retain their complete coverage even if only some bounds have
been refreshed. No monotonic improvement of the numerical values is needed
for this exclusion law; their stated validity is enough. -/
theorem partial_double_count_excludes_region (o : OrderedReduction V)
    (C : Atoms α → Prop) {prior current : Region α}
    (inside : FormulaBounds.Inside current prior) {ι : Type w}
    (terms : ι → List (Term α V)) (first second : List ι) (S : List (Term α V))
    (oldLower newLower oldUpper newUpper : ι → V)
    (lowerFresh upperFresh : ι → Prop)
    (firstCovers : (first.map terms).flatten.Perm S)
    (secondCovers : (second.map terms).flatten.Perm S)
    (oldBelow : ∀ i ∈ first, LowerIn o C prior (terms i) (oldLower i))
    (newBelow : ∀ i ∈ first, LowerIn o C current (terms i) (newLower i))
    (oldAbove : ∀ i ∈ second, UpperIn o C prior (terms i) (oldUpper i))
    (newAbove : ∀ i ∈ second, UpperIn o C current (terms i) (newUpper i))
    (exceeds : ¬ o.le
      (o.toReduction.fold (fun i => if lowerFresh i then newLower i else oldLower i) first)
      (o.toReduction.fold (fun i => if upperFresh i then newUpper i else oldUpper i) second)) :
    ∀ z, current.Contains z → ¬ C z := by
  classical
  let lowerRows : List (List (Term α V) × V) :=
    first.map fun i => (terms i, if lowerFresh i then newLower i else oldLower i)
  let upperRows : List (List (Term α V) × V) :=
    second.map fun i => (terms i, if upperFresh i then newUpper i else oldUpper i)
  have enclosing : ∀ {z : Atoms α}, current.Contains z → prior.Contains z := by
    intro z within
    exact ⟨fun a held => within.1 a (inside.1 a held),
      fun a present => inside.2 a (within.2 a present)⟩
  have lowerCovers : (lowerRows.map Prod.fst).flatten.Perm S := by
    simpa only [lowerRows, List.map_map, Function.comp_def] using firstCovers
  have upperCovers : (upperRows.map Prod.fst).flatten.Perm S := by
    simpa only [upperRows, List.map_map, Function.comp_def] using secondCovers
  have lower : ∀ row ∈ lowerRows, LowerIn o C current row.1 row.2 := by
    intro row listed
    obtain ⟨i, member, rfl⟩ := List.mem_map.mp listed
    intro z within belongs
    change o.le (if lowerFresh i then newLower i else oldLower i)
      (measure o.toReduction z (terms i))
    by_cases fresh : lowerFresh i
    · simpa only [if_pos fresh] using newBelow i member z within belongs
    · simpa only [if_neg fresh] using oldBelow i member z (enclosing within) belongs
  have upper : ∀ row ∈ upperRows, UpperIn o C current row.1 row.2 := by
    intro row listed
    obtain ⟨i, member, rfl⟩ := List.mem_map.mp listed
    intro z within belongs
    change o.le (measure o.toReduction z (terms i))
      (if upperFresh i then newUpper i else oldUpper i)
    by_cases fresh : upperFresh i
    · simpa only [if_pos fresh] using newAbove i member z within belongs
    · simpa only [if_neg fresh] using oldAbove i member z (enclosing within) belongs
  have incompatible : ¬ o.le (o.toReduction.fold Prod.snd lowerRows)
      (o.toReduction.fold Prod.snd upperRows) := by
    simpa only [lowerRows, upperRows, o.toReduction.fold_map] using exceeds
  exact double_count_excludes_region o C current S lowerRows upperRows
    lowerCovers upperCovers lower upper incompatible

/-- **The double count.** Two families of rows each cover the same terms, the
first with lower bounds and the second with upper bounds of the class. If the
lower bounds' fold does not lie below the upper bounds', the class has no member:
the pigeonhole principle, refuted by one comparison. -/
theorem double_count_excludes (o : OrderedReduction V) (C : Atoms α → Prop)
    (S : List (Term α V)) (first second : List (List (Term α V) × V))
    (firstCovers : (first.map Prod.fst).flatten.Perm S)
    (secondCovers : (second.map Prod.fst).flatten.Perm S)
    (lower : ∀ row ∈ first, LowerOf o C row.1 row.2)
    (upper : ∀ row ∈ second, UpperOf o C row.1 row.2)
    (exceeds : ¬ o.le (o.toReduction.fold Prod.snd first) (o.toReduction.fold Prod.snd second)) :
    ∀ M, ¬ C M := by
  intro M member
  let everything : Region α := ⟨Empty, Full⟩
  have inside : everything.Contains M := ⟨fun _ absent => absent.elim, fun _ _ => trivial⟩
  exact double_count_excludes_region o C everything S first second firstCovers secondCovers
    (fun row listed => lower_in_of_class o C everything (lower row listed))
    (fun row listed => upper_in_of_class o C everything (upper row listed))
    exceeds M inside member

/-- **A cut certificate.** In a bipartite system, some rows of the first family
have lower bounds and some rows of the second have upper bounds in a region. The
first rows' terms split into those shared with the second rows and those the
second rows miss; the second rows' terms are the shared ones and the rest. If the
first rows' lower bounds do not lie below the second rows' upper bounds combined
with the region's measure of the missed terms, the region contains no member of
the class. In that measure, held and open atoms count and cut atoms do not.

Proof. A member's measure of the first rows' terms is at least their bounds. It is
the measure of the shared terms combined with that of the missed ones. The shared
terms lie among the second rows' terms, whose measure the upper bounds cap, since
the rest's measure is at least the identity; the missed terms measure at most as
the atoms not cut do. -/
theorem cut_excludes_region (o : OrderedReduction V) (C : Atoms α → Prop) (c : Region α)
    (first second : List (List (Term α V) × V)) (shared missed rest : List (Term α V))
    (firstSplit : (first.map Prod.fst).flatten.Perm (shared ++ missed))
    (secondSplit : (second.map Prod.fst).flatten.Perm (shared ++ rest))
    (missedPositive : Positive o missed) (restPositive : Positive o rest)
    (lower : ∀ row ∈ first, LowerIn o C c row.1 row.2)
    (upper : ∀ row ∈ second, UpperIn o C c row.1 row.2)
    (exceeds : ¬ o.le (o.toReduction.fold Prod.snd first)
      (o.op (o.toReduction.fold Prod.snd second) (measure o.toReduction c.upper missed))) :
    ∀ z, c.Contains z → ¬ C z := by
  intro z inside member
  have fromBelow : o.le (o.toReduction.fold Prod.snd first)
      (o.op (measure o.toReduction z shared) (measure o.toReduction z missed)) := by
    rw [← measure_append, ← measure_perm o.toReduction z firstSplit, measure_rows]
    exact fold_le_fold o first (fun row listed => lower row listed z inside member)
  have sharedBelow : o.le (measure o.toReduction z shared) (o.toReduction.fold Prod.snd second) := by
    have withRest : o.le (measure o.toReduction z shared)
        (o.op (measure o.toReduction z shared) (measure o.toReduction z rest)) := by
      have widened := o.op_le_op _ _ _ _ (o.le_refl (measure o.toReduction z shared))
        (unit_le_measure o z rest restPositive)
      rwa [o.toReduction.op_unit] at widened
    have capped : o.le (o.op (measure o.toReduction z shared) (measure o.toReduction z rest))
        (o.toReduction.fold Prod.snd second) := by
      rw [← measure_append, ← measure_perm o.toReduction z secondSplit, measure_rows]
      exact fold_le_fold o second (fun row listed => upper row listed z inside member)
    exact o.le_trans _ _ _ withRest capped
  have missedBelow : o.le (measure o.toReduction z missed) (measure o.toReduction c.upper missed) :=
    measure_mono o inside.2 missed missedPositive
  exact exceeds (o.le_trans _ _ _ fromBelow (o.op_le_op _ _ _ _ sharedBelow missedBelow))

/-! ## Provenance -/

/-- Reassociating a conjunction preserves satisfaction in the original
interpretation. This is a reading of the source formula, not a source rewrite. -/
theorem conjunction_assoc_original (M : Atoms α) (F G H : Formula α) :
    Satisfies M (.conj (.conj F G) H) ↔ Satisfies M (.conj F (.conj G H)) :=
  and_assoc

/-- A nested conjunction holds exactly when every listed leaf holds. The
existing `FormulaBounds.ConjChain` records each leaf occurrence, including
repeated formulas, without restricting the formulas at those leaves.

Proof. A leaf is its singleton list. A conjunction joins two leaf lists, and
its satisfaction supplies both induction hypotheses. -/
theorem conjunction_chain_original (M : Atoms α) {F : Formula α}
    {leaves : List (Formula α)} (chain : FormulaBounds.ConjChain F leaves) :
    Satisfies M F ↔ ∀ L ∈ leaves, Satisfies M L := by
  induction chain with
  | leaf F =>
    constructor
    · intro holds L member
      have same : L = F := List.mem_singleton.mp member
      rw [same]
      exact holds
    · intro each
      exact each F (List.mem_singleton.mpr rfl)
  | node left right ihLeft ihRight =>
    constructor
    · intro holds L member
      rcases List.mem_append.mp member with inLeft | inRight
      · exact ihLeft.mp holds.1 L inLeft
      · exact ihRight.mp holds.2 L inRight
    · intro each
      exact ⟨ihLeft.mpr (fun L member => each L (List.mem_append_left _ member)),
        ihRight.mpr (fun L member => each L (List.mem_append_right _ member))⟩

/-- Reading a conjunction tree as its flat list of leaf occurrences preserves
original satisfaction, by the chain law and `ChoiceIntervals.all_classical`.
Neither this reading nor its list representation removes repeated leaves. -/
theorem conjunction_flatten_original (M : Atoms α) {F : Formula α}
    {leaves : List (Formula α)} (chain : FormulaBounds.ConjChain F leaves) :
    Satisfies M F ↔ Satisfies M (ChoiceIntervals.all leaves) :=
  (conjunction_chain_original M chain).trans (ChoiceIntervals.all_classical M leaves).symm

/-- The exact canonical truth formula, falsum implying falsum, holds in every
original interpretation. This statement does not recognize other tautologies. -/
theorem canonical_truth_original (M : Atoms α) :
    Satisfies M (.imp .bot .bot) :=
  fun impossible => impossible

/-- An implication with the exact canonical truth antecedent has its
consequent's original truth. The original formula and its reduct stay intact. -/
theorem truth_implication_original (M : Atoms α) (F : Formula α) :
    Satisfies M (.imp (.imp .bot .bot) F) ↔ Satisfies M F :=
  ⟨fun holds => holds (canonical_truth_original M), fun holds _ => holds⟩

/-- An asserted double-negated conjunction gives the original truth of every
leaf, by `Ferraris.double_neg_satisfies` and the chain law. This is only an
original-world statement: it does not erase double negation in a frozen reduct. -/
theorem double_negated_conjunction_original (M : Atoms α) {F : Formula α}
    {leaves : List (Formula α)} (chain : FormulaBounds.ConjChain F leaves) :
    Satisfies M (Ferraris.Neg (Ferraris.Neg F)) ↔ ∀ L ∈ leaves, Satisfies M L :=
  (Ferraris.double_neg_satisfies M F).trans (conjunction_chain_original M chain)

/-- **Provenance by patterns.** A property local to a set of atoms holds in every
model of a group of formulas that mention only those atoms, provided every
interpretation inside the set that violates the property violates some formula of
the group. For a group over k atoms that premise is a check of 2^k patterns; the
check is what a recognized row must pass, once, before it is used.

Proof. Restrict a model of the group to the set. The restriction agrees with the
model there, so it satisfies the group's formulas (`Ferraris.satisfies_congr`);
it therefore has the property, and so does the model, by locality. -/
theorem entailed_by_patterns (S : Atoms α) (G : Theory α) (P : Atoms α → Prop)
    (depends : ∀ M N, EqOn S M N → P M → P N)
    (mentions : ∀ F ∈ G, ∀ a, Mentions F a → S a)
    (checked : ∀ N, (∀ a, N a → S a) → ¬ P N → ∃ F ∈ G, ¬ Satisfies N F) :
    ∀ M, Models M G → P M := by
  intro M model
  let N : Atoms α := fun a => S a ∧ M a
  have agree : EqOn S M N := fun a inside => ⟨fun present => ⟨inside, present⟩, fun both => both.2⟩
  have back : EqOn S N M := fun a inside => (agree a inside).symm
  apply Classical.byContradiction
  intro lacking
  have restrictedLacks : ¬ P N := fun holds => lacking (depends N M back holds)
  obtain ⟨F, member, violated⟩ := checked N (fun _ present => present.1) restrictedLacks
  exact violated ((satisfies_congr agree F (mentions F member)).mp (model F member))

/-- An equation whose terms' atoms lie in a set is a property local to it. -/
theorem equation_local (r : Reduction V) (S : Atoms α) (e : Equation α V)
    (inside : ∀ t ∈ e.terms, S t.1) :
    ∀ M N, EqOn S M N → e.Holds r M → e.Holds r N := by
  intro M N agree holds
  show measure r N e.terms = e.constant
  rw [← measure_congr r agree e.terms inside]
  exact holds

/-- Every answer set of a theory models every group of formulas drawn from it, so
a property of the group's models holds in every answer set. -/
theorem entailed_by_part (T G : Theory α) (included : ∀ F ∈ G, F ∈ T) (P : Atoms α → Prop)
    (entailed : ∀ M, Models M G → P M) : ∀ M, Stable M T → P M :=
  fun M answer => entailed M (fun F member => answer.1 F (included F member))

/-- **One checked fact step.** A class which already entails the recorded
positive facts also entails the next atom when it entails an atomic-head
implication and the earlier facts suffice for that implication's body.

Proof. The earlier facts hold in the selected class member, hence its body
holds; the asserted implication gives its head. Earlier facts remain true.
Applying this law in witness order justifies each prefix. A readiness claim
about the final fact set instead would not establish its premise. No producer
uniqueness, complete interpretation or least-fixed-point hypothesis is used. -/
theorem entailed_fact_step (C : Atoms α → Prop) (known : List α)
    (body : Formula α) (head : α)
    (earlier : ∀ M, C M → ∀ a ∈ known, M a)
    (source : ∀ M, C M → Satisfies M (.imp body (.atom head)))
    (ready : ∀ M, (∀ a ∈ known, M a) → Satisfies M body) :
    ∀ M, C M → ∀ a ∈ head :: known, M a := by
  intro M member atom listed
  have earlierHolds : ∀ a ∈ known, M a := earlier M member
  have bodyHolds : Satisfies M body := ready M earlierHolds
  have headHolds : M head := source M member bodyHolds
  rcases List.mem_cons.mp listed with same | previous
  · subst atom
    exact headHolds
  · exact earlierHolds atom previous

/-- **Provenance by patterns under proved facts.** Every member of a consequence
class models the original theory and contains the checked positive facts. Each
residual clause follows, under those facts, from an original root. If the
residual clauses mention only S and exclude every local pattern violating P,
then every class member has P.

Proof. The facts and original model give each residual clause by its declared
correspondence. Apply `entailed_by_patterns` to this residual theory, which
really mentions only S. The original roots may mention additional fact atoms;
they are not falsely claimed to satisfy that locality premise. Neither the
original theory nor its frozen reduct is rewritten by this law. -/
theorem entailed_by_residual_patterns
    (C : Atoms α → Prop) (T residual : Theory α) (known : List α)
    (S : Atoms α) (P : Atoms α → Prop)
    (facts : ∀ M, C M → ∀ a ∈ known, M a)
    (originalModels : ∀ M, C M → Models M T)
    (correspondence : ∀ R ∈ residual, ∃ F ∈ T,
      ∀ M, (∀ a ∈ known, M a) → Satisfies M F → Satisfies M R)
    (depends : ∀ M N, EqOn S M N → P M → P N)
    (mentions : ∀ F ∈ residual, ∀ a, Mentions F a → S a)
    (checked : ∀ N, (∀ a, N a → S a) → ¬ P N →
      ∃ F ∈ residual, ¬ Satisfies N F) :
    ∀ M, C M → P M := by
  intro M member
  have residualModels : Models M residual := by
    intro clause listed
    obtain ⟨original, asserted, preserves⟩ := correspondence clause listed
    have sourceHolds : Satisfies M original := originalModels M member original asserted
    exact preserves M (facts M member) sourceHolds
  exact entailed_by_patterns S residual P depends mentions checked M residualModels

/-- A pattern is not forbidden by completion exactly when the head agrees with
some fully satisfied body. AND reduces each body's literal values, OR reduces
all producers, and XOR records disagreement. This is a pointwise statement, so
it applies separately to every column bit of a finite interpretation table. -/
theorem completion_fold_false_iff (head : Bool) (value : α → Bool)
    (bodies : List (List α)) :
    (head ^^ any.fold (all.fold value) bodies) = false ↔
      (head = true ↔ ∃ body ∈ bodies, ∀ literal ∈ body, value literal = true) := by
  have bodyUnion : any.fold (all.fold value) bodies = true ↔
      ∃ body ∈ bodies, ∀ literal ∈ body, value literal = true := by
    rw [any_fold_true_iff]
    constructor
    · rintro ⟨body, listed, holds⟩
      exact ⟨body, listed, (all_fold_true_iff value body).mp holds⟩
    · rintro ⟨body, listed, each⟩
      exact ⟨body, listed, (all_fold_true_iff value body).mpr each⟩
  rw [← bodyUnion]
  cases head <;> cases any.fold (all.fold value) bodies <;> decide

open DisjunctiveSupport in
/-- **Provenance by completion.** A property local to a set holding an atom and
the atoms of its rules' bodies holds in every answer set, provided every
interpretation inside the set that satisfies the atom's completion has it. The
atom must be defined only by normal rules with it as their sole head, under a
complete inventory (`DisjunctiveSupport.answer_set_completion`).

Proof. An answer set satisfies the completion. Its restriction to the set agrees
with it there, so satisfies the completion too (`Ferraris.satisfies_congr` for
the bodies), and so has the property; locality returns it to the answer set. -/
theorem entailed_by_completion (T : Theory α) (rules : List (Producer α))
    (choices : List (ChoiceProducer α)) (covered : CoveredWithChoices T rules choices)
    (original : ∀ r ∈ rules, r.formula ∈ T) (p : α) (bodies : List (Formula α))
    (producers : ∀ B ∈ bodies, Producer.rule B (.atom p) ∈ rules)
    (sole : ∀ r ∈ rules, r.head.Contains p → ∃ B ∈ bodies, r = .rule B (.atom p))
    (unchosen : ∀ c ∈ choices, c.head ≠ p)
    (S : Atoms α) (P : Atoms α → Prop) (depends : ∀ M N, EqOn S M N → P M → P N)
    (defined : S p) (mentions : ∀ B ∈ bodies, ∀ a, Mentions B a → S a)
    (checked : ∀ N, (∀ a, N a → S a) → (N p ↔ ∃ B ∈ bodies, Satisfies N B) → P N) :
    ∀ M, Stable M T → P M := by
  intro M answer
  have completion := answer_set_completion M T rules choices covered original p bodies
    producers sole unchosen answer
  let N : Atoms α := fun a => S a ∧ M a
  have agree : EqOn S M N := fun a inside => ⟨fun present => ⟨inside, present⟩, fun both => both.2⟩
  have back : EqOn S N M := fun a inside => (agree a inside).symm
  have restricted : N p ↔ ∃ B ∈ bodies, Satisfies N B := by
    constructor
    · intro present
      obtain ⟨B, listed, holds⟩ := completion.mp present.2
      exact ⟨B, listed, (satisfies_congr agree B (mentions B listed)).mp holds⟩
    · rintro ⟨B, listed, holds⟩
      exact ⟨defined, completion.mpr ⟨B, listed, (satisfies_congr agree B (mentions B listed)).mpr holds⟩⟩
  exact depends N M back (checked N (fun _ present => present.1) restricted)

/-- A guard releases a guarded constraint: where the guard holds, the guarded
formula fails. -/
theorem guard_releases (M : Atoms α) (guard Φ : Formula α)
    (constraint : Satisfies M (Ferraris.Neg (.conj guard Φ))) (guarded : Satisfies M guard) :
    ¬ Satisfies M Φ :=
  fun holds => constraint ⟨guarded, holds⟩

/-! ## Instances -/

/-- **Parity**: the Booleans under exclusive-or, an abelian group in which every
value is its own inverse. A term's measure is the parity of its atoms that hold. -/
def parity : AbelianGroup Bool where
  op := fun a b => a ^^ b
  unit := false
  assoc := fun a b c => by cases a <;> cases b <;> cases c <;> rfl
  comm := fun a b => by cases a <;> cases b <;> rfl
  unit_op := fun a => by cases a <;> rfl
  neg := fun a => a
  op_neg := fun a => by cases a <;> rfl

/-- **Odd multiplicity.** A list of unit parity terms has the same measure as
any duplicate-free list containing exactly its atoms of odd multiplicity.

Proof. Remove the first occurrence. If its atom is in the odd list, remove it
there too and use permutation invariance. Otherwise insert it into the odd list
for the induction hypothesis; the two copies then cancel. No ordering of atoms
or packed representation is assumed. -/
theorem parity_measure_odd [DecidableEq α] (M : Atoms α) (atoms mask : List α)
    (distinct : mask.Nodup)
    (odd : ∀ a, a ∈ mask ↔ atoms.count a % 2 = 1) :
    measure parity.toReduction M (atoms.map fun a => (a, true)) =
      measure parity.toReduction M (mask.map fun a => (a, true)) := by
  induction atoms generalizing mask with
  | nil =>
    have empty : mask = [] := by
      apply List.eq_nil_iff_forall_not_mem.mpr
      intro a member
      have impossible := (odd a).mp member
      simp at impossible
    rw [empty]
  | cons a rest ih =>
    by_cases present : a ∈ mask
    · have restOdd : ∀ b, b ∈ mask.erase a ↔ rest.count b % 2 = 1 := by
        intro b
        rw [distinct.mem_erase_iff]
        by_cases same : b = a
        · subst b
          have before : (rest.count a + 1) % 2 = 1 := by
            simpa only [List.count_cons_self] using (odd a).mp present
          simp only [ne_self_iff_false, false_and, false_iff]
          omega
        · simpa [same, Ne.symm same, List.count_cons] using odd b
      have reduced := ih (mask.erase a) (distinct.erase a) restOdd
      have reordered := measure_perm parity.toReduction M
        ((List.perm_cons_erase present).map (fun b => (b, true)))
      rw [reordered, List.map_cons, List.map_cons, measure_cons, measure_cons, reduced]
    · have restOdd : ∀ b, b ∈ a :: mask ↔ rest.count b % 2 = 1 := by
        intro b
        by_cases same : b = a
        · subst b
          have before : ¬ (a :: rest).count a % 2 = 1 :=
            fun value => present ((odd a).mpr value)
          simp only [List.count_cons_self] at before
          simp only [List.mem_cons, true_or, true_iff]
          omega
        · simpa [same, Ne.symm same, List.count_cons] using odd b
      have reduced := ih (a :: mask) (List.nodup_cons.mpr ⟨present, distinct⟩) restOdd
      rw [List.map_cons, measure_cons, reduced, List.map_cons, measure_cons]
      show parity.op (contribution parity.toReduction M (a, true))
        (parity.op (contribution parity.toReduction M (a, true))
          (measure parity.toReduction M (mask.map fun b => (b, true)))) = _
      rw [← parity.assoc]
      have pair : parity.op (contribution parity.toReduction M (a, true))
          (contribution parity.toReduction M (a, true)) = parity.unit := by
        have cancelled : measure parity.toReduction M [(a, true), (a, true)] = parity.unit :=
          measure_cancel parity M a true
        rw [measure_cons, measure_cons] at cancelled
        change parity.op (contribution parity.toReduction M (a, true))
          (parity.op (contribution parity.toReduction M (a, true)) parity.unit) = parity.unit
          at cancelled
        rwa [parity.toReduction.op_unit] at cancelled
      rw [pair, parity.unit_op]

/-- **XOR of finite masks.** If duplicate-free atom lists represent two parity
equations and a third list's membership bits are their pointwise exclusive-or,
the third list with the exclusive-or constant is an equation of the same class.

Proof. Add the original equations. The XOR mask lists exactly the odd-multiplicity
atoms of the concatenation; `parity_measure_odd` and `equation_of_agreement`
transport the sum to that mask. The premise is the representation obligation;
this theorem does not verify machine-word XOR, coordinates or padding. -/
theorem parity_equation_of_xor [DecidableEq α] (C : Atoms α → Prop)
    (first second combined : List α) (firstConstant secondConstant : Bool)
    (firstDistinct : first.Nodup) (secondDistinct : second.Nodup)
    (combinedDistinct : combined.Nodup)
    (bits : ∀ a, decide (a ∈ combined) = (decide (a ∈ first) ^^ decide (a ∈ second)))
    (left : EquationOf parity.toReduction C
      ⟨first.map (fun a => (a, true)), firstConstant⟩)
    (right : EquationOf parity.toReduction C
      ⟨second.map (fun a => (a, true)), secondConstant⟩) :
    EquationOf parity.toReduction C
      ⟨combined.map (fun a => (a, true)), firstConstant ^^ secondConstant⟩ := by
  have odd : ∀ a, a ∈ combined ↔ (first ++ second).count a % 2 = 1 := by
    intro a
    have decoded : a ∈ combined ↔
        (decide (a ∈ first) ^^ decide (a ∈ second)) = true := by
      rw [← bits a]
      simp
    rw [List.count_append, firstDistinct.count, secondDistinct.count]
    by_cases inFirst : a ∈ first <;> by_cases inSecond : a ∈ second <;>
      simpa [inFirst, inSecond] using decoded
  apply equation_of_agreement parity.toReduction C (equation_add parity.toReduction C left right)
  · intro M _
    simpa only [Equation.add, List.map_append] using
      (parity_measure_odd M (first ++ second) combined combinedDistinct odd).symm
  · rfl

/-- **Counts**: the natural numbers under addition and their order. A term's
measure is the weighted count of its atoms that hold. -/
def counting : OrderedReduction Nat where
  op := fun a b => a + b
  unit := 0
  assoc := Nat.add_assoc
  comm := Nat.add_comm
  unit_op := Nat.zero_add
  le := fun a b => a ≤ b
  le_refl := Nat.le_refl
  le_trans := fun _ _ _ => Nat.le_trans
  op_le_op := fun _ _ _ _ => Nat.add_le_add

/-- **Replace one occurrence in a finite count.** The old value is a summand
of the complete natural fold, so subtracting it does not truncate; adding the
new value gives exactly the fold with that occurrence replaced. Equal-valued
occurrences elsewhere in the list remain untouched.

Proof. Split both folds into the prefix, selected occurrence and suffix.
Natural addition gives the old summand's lower bound and cancels that summand
in the replacement equation. These are exact natural values; finite-width
representability and any refusal before a machine operation are separate
premises of a concrete implementation. -/
theorem counting_replace_occurrence (preceding suffix : List Nat) (old new : Nat) :
    old ≤ counting.toReduction.fold id (preceding ++ old :: suffix) ∧
      counting.toReduction.fold id (preceding ++ old :: suffix) - old + new =
        counting.toReduction.fold id (preceding ++ new :: suffix) := by
  have previous : counting.toReduction.fold id (preceding ++ old :: suffix) =
      counting.toReduction.fold id preceding +
        (old + counting.toReduction.fold id suffix) := by
    rw [counting.toReduction.fold_append, counting.toReduction.fold_cons]
    rfl
  have replaced : counting.toReduction.fold id (preceding ++ new :: suffix) =
      counting.toReduction.fold id preceding +
        (new + counting.toReduction.fold id suffix) := by
    rw [counting.toReduction.fold_append, counting.toReduction.fold_cons]
    rfl
  rw [previous, replaced]
  constructor <;> omega

/-- Folding one for each member with natural addition gives the list's length.
This counts occurrences, including repetitions, and the empty list contributes
zero. Reading the result as a family's cardinality additionally requires a
complete list without duplicate members.

Proof. The empty fold is zero. Adding a head contributes one to the tail's
count, which the induction hypothesis identifies with its length. -/
theorem counting_fold_one (members : List α) :
    counting.toReduction.fold (fun _ => 1) members = members.length := by
  induction members with
  | nil => rfl
  | cons head rest counted =>
    rw [counting.toReduction.fold_cons, counted]
    show 1 + rest.length = (head :: rest).length
    simp only [List.length_cons, Nat.add_comm]

/-- A count over atoms none of which holds is zero. -/
theorem count_of_none (M : Atoms α) (S : List α) (none : ∀ a ∈ S, ¬ M a) :
    measure counting.toReduction M (S.map fun a => (a, 1)) = 0 := by
  induction S with
  | nil => rfl
  | cons head rest counted =>
    have absent : ¬ M head := none head (List.mem_cons_self ..)
    rw [List.map_cons, measure_cons,
      contribution_of_fails counting.toReduction (M := M) (t := (head, 1)) absent,
      counted (fun a listed => none a (List.mem_cons_of_mem head listed))]
    rfl

/-- **A clique is an upper bound.** If no member of the class holds two distinct
atoms of a duplicate-free list `S`, every member holds at most one of them: the
count over `S` is at most one. Each pair's premise is a refuted two-atom pattern
(`Algebra.entailed_by_refutations`). -/
theorem upper_of_clique (C : Atoms α → Prop) (S : List α) (nodup : S.Nodup)
    (excluded : ∀ a ∈ S, ∀ b ∈ S, a ≠ b → ∀ M, C M → ¬ (M a ∧ M b)) :
    UpperOf counting C (S.map fun a => (a, 1)) 1 := by
  classical
  intro M member
  induction S with
  | nil => exact Nat.zero_le 1
  | cons head rest counted =>
    have headFresh : head ∉ rest := (List.nodup_cons.mp nodup).1
    have restNodup : rest.Nodup := (List.nodup_cons.mp nodup).2
    have restExcluded : ∀ a ∈ rest, ∀ b ∈ rest, a ≠ b → ∀ N, C N → ¬ (N a ∧ N b) :=
      fun a inRest b inRest' distinct =>
        excluded a (List.mem_cons_of_mem head inRest) b (List.mem_cons_of_mem head inRest') distinct
    rw [List.map_cons, measure_cons]
    by_cases held : M head
    · have none : ∀ a ∈ rest, ¬ M a := by
        intro a listed holds
        have distinct : head ≠ a := by
          intro same
          rw [same] at headFresh
          exact headFresh listed
        exact excluded head (List.mem_cons_self ..) a (List.mem_cons_of_mem head listed) distinct
          M member ⟨held, holds⟩
      rw [contribution_of_holds counting.toReduction (M := M) (t := (head, 1)) held,
        count_of_none M rest none]
      exact Nat.le_refl 1
    · rw [contribution_of_fails counting.toReduction (M := M) (t := (head, 1)) held]
      have tail : measure counting.toReduction M (rest.map fun a => (a, 1)) ≤ 1 :=
        counted restNodup restExcluded
      show 0 + measure counting.toReduction M (rest.map fun a => (a, 1)) ≤ 1
      rw [Nat.zero_add]
      exact tail

/-- A count over atoms all of which hold is their number. -/
theorem count_of_all (M : Atoms α) (S : List α) (all : ∀ a ∈ S, M a) :
    measure counting.toReduction M (S.map fun a => (a, 1)) = S.length := by
  induction S with
  | nil => rfl
  | cons head rest counted =>
    have present : M head := all head (List.mem_cons_self ..)
    rw [List.map_cons, measure_cons,
      contribution_of_holds counting.toReduction (M := M) (t := (head, 1)) present,
      counted (fun a listed => all a (List.mem_cons_of_mem head listed))]
    show 1 + rest.length = (head :: rest).length
    simp only [List.length_cons, Nat.add_comm]

/-- **A clique of covering pairs is a lower bound.** If every member of the class
holds at least one of any two distinct atoms of a duplicate-free list `S`, every
member misses at most one of them: the count over `S` is at least its length less
one. Each pair's premise is a refuted two-atom pattern, the one cutting both
(`Algebra.entailed_by_refutations`). This is `upper_of_clique` with holding and
cutting exchanged, so a pairwise constraint yields a bound whichever polarity its
atoms were read in.

Proof. By induction on `S`. A member holding the head counts it together with all
but at most one of the rest, by the induction hypothesis. A member missing the
head holds every other atom, each being paired with the head, and so counts the
whole rest (`count_of_all`). -/
theorem lower_of_cover (C : Atoms α → Prop) (S : List α) (nodup : S.Nodup)
    (covered : ∀ a ∈ S, ∀ b ∈ S, a ≠ b → ∀ M, C M → M a ∨ M b) :
    LowerOf counting C (S.map fun a => (a, 1)) (S.length - 1) := by
  classical
  intro M member
  induction S with
  | nil => exact Nat.zero_le _
  | cons head rest counted =>
    have headFresh : head ∉ rest := (List.nodup_cons.mp nodup).1
    have restNodup : rest.Nodup := (List.nodup_cons.mp nodup).2
    have restCovered : ∀ a ∈ rest, ∀ b ∈ rest, a ≠ b → ∀ N, C N → N a ∨ N b :=
      fun a inRest b inRest' distinct =>
        covered a (List.mem_cons_of_mem head inRest) b (List.mem_cons_of_mem head inRest') distinct
    rw [List.map_cons, measure_cons]
    by_cases held : M head
    · have tail : rest.length - 1 ≤ measure counting.toReduction M (rest.map fun a => (a, 1)) :=
        counted restNodup restCovered
      rw [contribution_of_holds counting.toReduction (M := M) (t := (head, 1)) held]
      show (head :: rest).length - 1 ≤
        1 + measure counting.toReduction M (rest.map fun a => (a, 1))
      simp only [List.length_cons]
      omega
    · have all : ∀ a ∈ rest, M a := by
        intro a listed
        have distinct : head ≠ a := by
          intro same
          rw [same] at headFresh
          exact headFresh listed
        have one : M head ∨ M a :=
          covered head (List.mem_cons_self ..) a (List.mem_cons_of_mem head listed) distinct
            M member
        exact one.resolve_left held
      rw [contribution_of_fails counting.toReduction (M := M) (t := (head, 1)) held,
        count_of_all M rest all]
      show (head :: rest).length - 1 ≤ 0 + rest.length
      simp only [List.length_cons]
      omega

/-- **Bounds from allowed patterns.** If every member of the class is allowed, where
being allowed depends on `S` alone, and every allowed interpretation supported on
`S` counts between `low` and `high` over `S`, then those are a lower and an upper
bound of the class.

Proof. A member and its restriction to `S` agree on `S`, so they count alike
(`measure_congr`); the restriction is allowed and supported on `S`. -/
theorem bounds_of_allowed (C : Atoms α → Prop) (S : List α) (Allowed : Atoms α → Prop)
    (depends : ∀ σ τ : Atoms α, (∀ a ∈ S, (σ a ↔ τ a)) → Allowed σ → Allowed τ)
    (entailed : ∀ M, C M → Allowed M) (low high : Nat)
    (bounded : ∀ σ : Atoms α, Allowed σ → (∀ a, σ a → a ∈ S) →
      low ≤ measure counting.toReduction σ (S.map fun a => (a, 1)) ∧
        measure counting.toReduction σ (S.map fun a => (a, 1)) ≤ high) :
    LowerOf counting C (S.map fun a => (a, 1)) low ∧
      UpperOf counting C (S.map fun a => (a, 1)) high := by
  have restricted : ∀ M, C M →
      low ≤ measure counting.toReduction M (S.map fun a => (a, 1)) ∧
        measure counting.toReduction M (S.map fun a => (a, 1)) ≤ high := by
    intro M member
    have agree : ∀ a ∈ S, ((fun b => b ∈ S ∧ M b) a ↔ M a) :=
      fun a listed => ⟨fun restricted => restricted.2, fun held => ⟨listed, held⟩⟩
    have allowed : Allowed (fun b => b ∈ S ∧ M b) :=
      depends M _ (fun a listed => (agree a listed).symm) (entailed M member)
    have supported : ∀ a, (fun b => b ∈ S ∧ M b) a → a ∈ S := fun _ restricted => restricted.1
    have same : measure counting.toReduction M (S.map fun a => (a, 1)) =
        measure counting.toReduction (fun b => b ∈ S ∧ M b) (S.map fun a => (a, 1)) := by
      apply measure_congr counting.toReduction (S := fun a => a ∈ S)
      · intro a listed
        exact (agree a listed).symm
      · intro t listed
        rcases List.mem_map.mp listed with ⟨a, inS, rfl⟩
        exact inS
    rw [same]
    exact bounded _ allowed supported
  exact ⟨fun M member => (restricted M member).1, fun M member => (restricted M member).2⟩

/-- Encoding every nonnegative contribution before folding gives the encoding
of the exact natural sum. Complete enumeration, membership and distinctness
are separate premises whenever that sum is read as a family cardinality.

Proof. Both empty folds encode zero. At a head, the induction hypothesis gives
the encoded tail and bounded addition commutes with encoding the exact sum. -/
theorem bounded_counting_fold (limit : Nat) (value : α → Nat) (members : List α) :
    (AxisFolds.BoundedSum.reduction limit).fold
        (fun member => AxisFolds.BoundedSum.encode limit (value member)) members =
      AxisFolds.BoundedSum.encode limit (counting.toReduction.fold value members) := by
  induction members with
  | nil => rfl
  | cons head rest counted =>
    rw [(AxisFolds.BoundedSum.reduction limit).fold_cons,
      counting.toReduction.fold_cons, counted]
    exact AxisFolds.BoundedSum.op_encode limit (value head)
      (counting.toReduction.fold value rest)

/-- A bounded count is exact list length when representable and overflow
otherwise. Repeated members still count repeatedly; no set-cardinality or
coverage conclusion is inferred from this value law. -/
theorem bounded_counting_fold_one (limit : Nat) (members : List α) :
    (AxisFolds.BoundedSum.reduction limit).fold
        (fun _ => AxisFolds.BoundedSum.encode limit 1) members =
      AxisFolds.BoundedSum.encode limit members.length := by
  calc
    (AxisFolds.BoundedSum.reduction limit).fold
        (fun _ => AxisFolds.BoundedSum.encode limit 1) members =
        AxisFolds.BoundedSum.encode limit (counting.toReduction.fold (fun _ => 1) members) :=
      bounded_counting_fold limit (fun _ => 1) members
    _ = AxisFolds.BoundedSum.encode limit members.length :=
      congrArg (AxisFolds.BoundedSum.encode limit) (counting_fold_one members)

/-- Overflow of the complete occurrence count is exactly excess list length. -/
theorem bounded_counting_overflow_iff (limit : Nat) (members : List α) :
    (AxisFolds.BoundedSum.reduction limit).fold
        (fun _ => AxisFolds.BoundedSum.encode limit 1) members = none ↔
      limit < members.length := by
  rw [bounded_counting_fold_one]
  exact AxisFolds.BoundedSum.encode_overflow_iff limit members.length

/-- In parity an atom of odd coefficient is forced, under literals giving every
other atom of an equation its value in σ, to the constant plus the others'
parity in σ. -/
theorem parity_forces [DecidableEq α] (C : Atoms α → Prop) (e : Equation α Bool)
    (of : EquationOf parity.toReduction C e) (x : α) (σ : Atoms α) (B : List (Literal α))
    (given : ∀ t ∈ others x e.terms, (t.1, truth σ t.1) ∈ B)
    (odd : coefficient parity.toReduction x e.terms = true) :
    RowOf C B [(x, e.constant ^^ measure parity.toReduction σ (others x e.terms))] := by
  cases forced : (e.constant ^^ measure parity.toReduction σ (others x e.terms))
  · exact equation_forces_false parity C e of x σ B given forced (by rw [odd]; decide)
  · exact equation_forces_true parity C e of x σ B given (forced.trans odd.symm)
      (by rw [odd]; decide)

open DisjunctiveSupport in
/-- The two-rule exclusive-or definition `p :- a, not b. p :- not a, b.`, under a
complete inventory with no other producer of `p` and no choice of it, gives
every answer set the parity equation `p + a + b = 0`: an instance of
`DisjunctiveSupport.answer_set_completion`. -/
theorem xor_definition (T : Theory α) (rules : List (Producer α))
    (choices : List (ChoiceProducer α)) (covered : CoveredWithChoices T rules choices)
    (original : ∀ r ∈ rules, r.formula ∈ T) (p a b : α)
    (first : Producer.rule (.conj (.atom a) (Ferraris.Neg (.atom b))) (.atom p) ∈ rules)
    (second : Producer.rule (.conj (Ferraris.Neg (.atom a)) (.atom b)) (.atom p) ∈ rules)
    (sole : ∀ r ∈ rules, r.head.Contains p →
      r = .rule (.conj (.atom a) (Ferraris.Neg (.atom b))) (.atom p) ∨
        r = .rule (.conj (Ferraris.Neg (.atom a)) (.atom b)) (.atom p))
    (unchosen : ∀ c ∈ choices, c.head ≠ p) :
    EquationOf parity.toReduction (fun M => Stable M T) ⟨[(p, true), (a, true), (b, true)], false⟩ := by
  intro M answer
  have completion := answer_set_completion M T rules choices covered original p
    [.conj (.atom a) (Ferraris.Neg (.atom b)), .conj (Ferraris.Neg (.atom a)) (.atom b)]
    (by simp [first, second])
    (fun r member contains => by
      rcases sole r member contains with same | same
      · exact ⟨_, by simp, same⟩
      · exact ⟨_, by simp, same⟩)
    unchosen answer
  have defined : M p ↔ ((M a ∧ ¬ M b) ∨ (¬ M a ∧ M b)) := by
    simpa [Satisfies, Ferraris.Neg] using completion
  show measure parity.toReduction M [(p, true), (a, true), (b, true)] = false
  simp only [measure, Reduction.fold]
  by_cases pa : M a <;> by_cases pb : M b <;> by_cases pp : M p <;>
    simp_all [contribution, parity, Reduction.foldFrom]

/-- A threshold over atom conditions measures its terms: its active weights add to
the count, under `counting`, of the terms pairing each position's atom with its
weight. -/
theorem threshold_measure {β : Type w} (conditions : β → Formula α) (atomOf : β → α)
    (atoms : ∀ i, conditions i = .atom (atomOf i)) (inputs : List (Nat × β)) (M : Atoms α) :
    ThresholdReduct.maskedSum (ThresholdReduct.active M conditions) inputs =
      measure counting.toReduction M (inputs.map fun input => (atomOf input.2, input.1)) := by
  induction inputs with
  | nil => rfl
  | cons input rest ih =>
    obtain ⟨weight, index⟩ := input
    have extended : ThresholdReduct.maskedSum (ThresholdReduct.active M conditions)
        ((weight, index) :: rest) =
          (if ThresholdReduct.active M conditions index then weight else 0) +
            ThresholdReduct.maskedSum (ThresholdReduct.active M conditions) rest := rfl
    rw [extended, ih, List.map_cons, measure_cons]
    show _ = contribution counting.toReduction M (atomOf index, weight) + _
    by_cases holds : M (atomOf index)
    · have active : ThresholdReduct.active M conditions index = true := by
        simp [ThresholdReduct.active, atoms index, Satisfies, holds]
      rw [active, contribution_of_holds counting.toReduction (t := (atomOf index, weight)) holds]
      rfl
    · have inactive : ThresholdReduct.active M conditions index = false := by
        simp [ThresholdReduct.active, atoms index, Satisfies, holds]
      rw [inactive, contribution_of_fails counting.toReduction (t := (atomOf index, weight)) holds]
      rfl

/-- A satisfied threshold over atom conditions bounds its terms' count from below. -/
theorem lower_of_threshold {β : Type w} (conditions : β → Formula α) (atomOf : β → α)
    (atoms : ∀ i, conditions i = .atom (atomOf i)) (inputs : List (Nat × β)) (bound : Nat)
    (M : Atoms α) (holds : Satisfies M (ThresholdReduct.formula conditions inputs bound)) :
    bound ≤ measure counting.toReduction M (inputs.map fun input => (atomOf input.2, input.1)) := by
  rw [← threshold_measure conditions atomOf atoms inputs M]
  exact (ThresholdReduct.original M conditions inputs bound).mp holds

/-- A negated threshold over atom conditions bounds its terms' count from above. -/
theorem upper_of_negated_threshold {β : Type w} (conditions : β → Formula α) (atomOf : β → α)
    (atoms : ∀ i, conditions i = .atom (atomOf i)) (inputs : List (Nat × β)) (bound : Nat)
    (M : Atoms α) (holds : Satisfies M (Ferraris.Neg (ThresholdReduct.formula conditions inputs bound))) :
    measure counting.toReduction M (inputs.map fun input => (atomOf input.2, input.1)) < bound := by
  rw [← threshold_measure conditions atomOf atoms inputs M]
  exact Nat.lt_of_not_le (fun reached =>
    holds ((ThresholdReduct.original M conditions inputs bound).mpr reached))

/-- An asserted conjunction supplies both lower bounds from positive threshold
leaves and strict upper bounds from negated threshold leaves. Other leaves may
occur between them; each consequence requires its own leaf membership.

Proof. The conjunction-chain law supplies each leaf's original satisfaction.
`lower_of_threshold` and `upper_of_negated_threshold` then give the two kinds of
bound. An optional outer double negation can supply the premise through
`Ferraris.double_neg_satisfies`. In an answer set, an asserted original root's
satisfaction comes from `Stable`'s original-model premise. No leaf is rewritten
or discarded, and this does not establish a recognizer's complete traversal. -/
theorem bounds_of_asserted_conjunction {β : Type w}
    (conditions : β → Formula α) (atomOf : β → α)
    (atoms : ∀ i, conditions i = .atom (atomOf i)) (M : Atoms α)
    {F : Formula α} {leaves : List (Formula α)}
    (chain : FormulaBounds.ConjChain F leaves) (asserted : Satisfies M F) :
    (∀ inputs bound, ThresholdReduct.formula conditions inputs bound ∈ leaves →
      bound ≤ measure counting.toReduction M
        (inputs.map fun input => (atomOf input.2, input.1))) ∧
    (∀ inputs bound, Ferraris.Neg (ThresholdReduct.formula conditions inputs bound) ∈ leaves →
      measure counting.toReduction M
        (inputs.map fun input => (atomOf input.2, input.1)) < bound) := by
  have each : ∀ L ∈ leaves, Satisfies M L :=
    (conjunction_chain_original M chain).mp asserted
  constructor
  · intro inputs bound member
    exact lower_of_threshold conditions atomOf atoms inputs bound M (each _ member)
  · intro inputs bound member
    exact upper_of_negated_threshold conditions atomOf atoms inputs bound M (each _ member)

/-- A count is a measure under `counting` with unit weights, bridging
`ChoiceIntervals.count` and `PartitionCapacities` to this module. -/
theorem count_measure (M : Atoms α) (atoms : List α) :
    ChoiceIntervals.count M atoms = measure counting.toReduction M (atoms.map fun a => (a, 1)) := by
  classical
  induction atoms with
  | nil => rfl
  | cons a rest ih =>
    rw [ChoiceIntervals.count_cons, ih, List.map_cons, measure_cons]
    by_cases holds : M a
    · rw [if_pos holds, contribution_of_holds counting.toReduction (t := (a, 1)) holds]
      show 1 + _ = 1 + _
      rfl
    · rw [if_neg holds, contribution_of_fails counting.toReduction (t := (a, 1)) holds]
      show 0 + _ = 0 + _
      rfl

/-- A satisfied clause forbidding all its atoms from being false bounds their
unit-weight measure from below by one. Repeated atoms do not invalidate this
lower bound; a recognizer may impose distinctness to represent a set of atoms.

Proof. If no listed atom held, every negated atom in the clause's conjunction
would hold, contradicting the clause. A held atom contributes to the count,
which `count_measure` identifies with the measure. -/
theorem lower_of_clause (M : Atoms α) (atoms : List α)
    (holds : Satisfies M
      (Ferraris.Neg (ChoiceIntervals.all (atoms.map fun a => Ferraris.Neg (.atom a))))) :
    1 ≤ measure counting.toReduction M (atoms.map fun a => (a, 1)) := by
  classical
  have present : ∃ a ∈ atoms, M a := by
    apply Classical.byContradiction
    intro absent
    apply holds
    apply (ChoiceIntervals.all_classical M _).mpr
    intro F listed
    obtain ⟨a, member, rfl⟩ := List.mem_map.mp listed
    exact fun satisfied => absent ⟨a, member, satisfied⟩
  obtain ⟨a, member, satisfied⟩ := present
  have counted : a ∈ atoms.filter (fun b => decide (M b)) :=
    List.mem_filter.mpr ⟨member, decide_eq_true satisfied⟩
  rw [← count_measure]
  exact List.length_pos_of_mem counted

/-- The double count in `PartitionCapacities`' terms, the instance C0 checks: two
families of groups whose members are each a permutation of the same atoms, the
first with lower bounds and the second with capacities, admit no interpretation
satisfying every bound when the lower bounds add to more than the capacities.

Proof. The first family's bounds add to at most the count of its members
(`PartitionCapacities.count_members`), which is the count of the atoms; the
second family's capacities bound that count from above
(`PartitionCapacities.capacity_bounds`). -/
theorem double_count_groups (S : List α) (first second : List (PartitionCapacities.Group α))
    (firstCovers : (PartitionCapacities.members first).Perm S)
    (secondCovers : (PartitionCapacities.members second).Perm S)
    (exceeds : PartitionCapacities.capacity second < (first.map Prod.snd).sum) (M : Atoms α) :
    ¬ ((∀ g ∈ first, g.2 ≤ ChoiceIntervals.count M g.1) ∧
      (∀ g ∈ second, ChoiceIntervals.count M g.1 ≤ g.2)) := by
  rintro ⟨lower, upper⟩
  have recount : ∀ {l₁ l₂ : List α}, l₁.Perm l₂ →
      ChoiceIntervals.count M l₁ = ChoiceIntervals.count M l₂ := by
    intro l₁ l₂ same
    rw [count_measure, count_measure]
    exact measure_perm counting.toReduction M (same.map _)
  have boundsBelow : ∀ rows : List (PartitionCapacities.Group α),
      (∀ g ∈ rows, g.2 ≤ ChoiceIntervals.count M g.1) →
        (rows.map Prod.snd).sum ≤ (rows.map (fun g => ChoiceIntervals.count M g.1)).sum := by
    intro rows bounded
    induction rows with
    | nil => exact Nat.le_refl 0
    | cons g rest ih =>
      simp only [List.map_cons, List.sum_cons]
      exact Nat.add_le_add (bounded g (List.mem_cons_self ..))
        (ih (fun h member => bounded h (List.mem_cons_of_mem g member)))
  have fromBelow : (first.map Prod.snd).sum ≤ ChoiceIntervals.count M S := by
    rw [← recount firstCovers, PartitionCapacities.count_members]
    exact boundsBelow first lower
  have fromAbove : ChoiceIntervals.count M S ≤ PartitionCapacities.capacity second := by
    rw [← recount secondCovers]
    exact PartitionCapacities.capacity_bounds M second upper
  omega

end Zetesis.AlgebraicRows
