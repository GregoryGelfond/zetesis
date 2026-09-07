import Zetesis.QueryCompaction

/-!
# Boolean-domain propagation for one immutable classical query

Each local constraint retains values having a compatible complete relation row.
Narrowing and every finite composition preserve all original query completions.
False frozen gates constrain their output to false and do not impose their
disabled connective on the inputs. Subset domains and strict removal concern
semantic atoms, not auxiliary nodes or objective restrictions.

This is a denotational finite-relation contract, not a WGSL/Rust implementation
proof. It proves neither a propagation scheduler's convergence nor complete
search. A quiescent nonempty domain store can still describe an unsatisfiable
query; it is residual information, not a stability or satisfiability decision.
-/

namespace Zetesis.Propagation

universe u v
variable {α : Type u}

abbrev Valuation (α : Type u) := α → Bool
abbrev Domains (α : Type u) := α → Bool → Prop

def Fits (D : Domains α) (v : Valuation α) : Prop := ∀ a, D a (v a)
def Narrower (D E : Domains α) : Prop := ∀ a b, D a b → E a b
def full : Domains α := fun _ _ => True

/-- Finite input positions may alias the same global variable. Projection
    deliberately permits extra inconsistent local rows; this can weaken
    propagation but never removes a consistent global completion. -/
structure Constraint (α : Type u) where
  arity : Nat
  slot : Fin arity → α
  relation : (Fin arity → Bool) → Prop

def Holds (c : Constraint α) (v : Valuation α) : Prop :=
  c.relation (fun i => v (c.slot i))

def Supported (c : Constraint α) (D : Domains α) (i : Fin c.arity) (b : Bool) : Prop :=
  ∃ row, (∀ j, D (c.slot j) (row j)) ∧ c.relation row ∧ row i = b

def narrow (c : Constraint α) (D : Domains α) : Domains α := fun a b =>
  D a b ∧ ∀ i, c.slot i = a → Supported c D i b

theorem narrow_decreases (c : Constraint α) (D : Domains α) :
    Narrower (narrow c D) D := fun _ _ h => h.1

theorem narrow_retains_completion (c : Constraint α) (D : Domains α)
    (v : Valuation α) (fits : Fits D v) (valid : Holds c v) :
    Fits (narrow c D) v := by
  intro a
  refine ⟨fits a, ?_⟩
  intro i same
  exact ⟨fun j => v (c.slot j), fun j => fits (c.slot j), valid,
    congrArg v same⟩

theorem narrow_models_iff (c : Constraint α) (D : Domains α) (v : Valuation α) :
    Fits (narrow c D) v ∧ Holds c v ↔ Fits D v ∧ Holds c v := by
  constructor
  · rintro ⟨fits, valid⟩
    exact ⟨fun a => (fits a).1, valid⟩
  · rintro ⟨fits, valid⟩
    exact ⟨narrow_retains_completion c D v fits valid, valid⟩

theorem narrow_monotone (c : Constraint α) (D E : Domains α) (sub : Narrower D E) :
    Narrower (narrow c D) (narrow c E) := by
  rintro a b ⟨inside, support⟩
  refine ⟨sub a b inside, ?_⟩
  intro i same
  obtain ⟨row, fits, valid, value⟩ := support i same
  exact ⟨row, fun j => sub _ _ (fits j), valid, value⟩

def Models (cs : List (Constraint α)) (v : Valuation α) : Prop :=
  ∀ c, c ∈ cs → Holds c v

/-- One finite ordered sweep; no fixed-point or exhaustion claim is implicit. -/
def sweep : List (Constraint α) → Domains α → Domains α
  | [], D => D
  | c :: cs, D => sweep cs (narrow c D)

theorem sweep_decreases (cs : List (Constraint α)) (D : Domains α) :
    Narrower (sweep cs D) D := by
  induction cs generalizing D with
  | nil => exact fun _ _ h => h
  | cons c cs ih =>
    exact fun a b h => (ih (narrow c D) a b h).1

theorem sweep_retains_completion (cs : List (Constraint α)) (D : Domains α)
    (v : Valuation α) (fits : Fits D v) (valid : Models cs v) :
    Fits (sweep cs D) v := by
  induction cs generalizing D with
  | nil => exact fits
  | cons c cs ih =>
    apply ih (narrow c D) (narrow_retains_completion c D v fits (valid c (by simp)))
    exact fun other member => valid other (by simp [member])

theorem sweep_models_iff (cs : List (Constraint α)) (D : Domains α) (v : Valuation α) :
    Fits (sweep cs D) v ∧ Models cs v ↔ Fits D v ∧ Models cs v := by
  constructor
  · rintro ⟨fits, valid⟩
    exact ⟨fun a => sweep_decreases cs D a (v a) (fits a), valid⟩
  · rintro ⟨fits, valid⟩
    exact ⟨sweep_retains_completion cs D v fits valid, valid⟩

def sweeps (cs : List (Constraint α)) : Nat → Domains α → Domains α
  | 0, D => D
  | n + 1, D => sweeps cs n (sweep cs D)

theorem finite_sweeps_models_iff (cs : List (Constraint α)) (n : Nat)
    (D : Domains α) (v : Valuation α) :
    Fits (sweeps cs n D) v ∧ Models cs v ↔ Fits D v ∧ Models cs v := by
  induction n generalizing D with
  | zero => rfl
  | succ n ih =>
    change Fits (sweeps cs n (sweep cs D)) v ∧ Models cs v ↔ _
    rw [ih, sweep_models_iff]

theorem empty_domain_refutes (cs : List (Constraint α)) (n : Nat) (D : Domains α)
    (a : α) (empty : ∀ b, ¬ sweeps cs n D a b) :
    ¬ ∃ v, Fits D v ∧ Models cs v := by
  rintro ⟨v, fits, valid⟩
  have retained := ((finite_sweeps_models_iff cs n D v).mpr ⟨fits, valid⟩).1
  exact empty (v a) (retained a)

/-- A parallel round intersects projections computed from the same snapshot. -/
def parallelRound (cs : List (Constraint α)) (D : Domains α) : Domains α := fun a b =>
  D a b ∧ ∀ c, c ∈ cs → narrow c D a b

theorem parallel_round_models_iff (cs : List (Constraint α)) (D : Domains α)
    (v : Valuation α) :
    Fits (parallelRound cs D) v ∧ Models cs v ↔ Fits D v ∧ Models cs v := by
  constructor
  · rintro ⟨fits, valid⟩
    exact ⟨fun a => (fits a).1, valid⟩
  · rintro ⟨fits, valid⟩
    refine ⟨?_, valid⟩
    intro a
    exact ⟨fits a, fun c member => narrow_retains_completion c D v fits (valid c member) a⟩

/-- A delayed projection from an older, larger domain snapshot is still safe
    when intersected with current domains. The query relation must be unchanged. -/
theorem stale_projection_retains_completion (c : Constraint α) (current old : Domains α)
    (sub : Narrower current old) (v : Valuation α)
    (fits : Fits current v) (valid : Holds c v) :
    Fits (fun a b => current a b ∧ narrow c old a b) v := by
  have oldFits : Fits old v := fun a => sub a (v a) (fits a)
  exact fun a => ⟨fits a, narrow_retains_completion c old v oldFits valid a⟩

inductive Operation where
  | and | or | imp
deriving DecidableEq

def gateValue : Operation → Bool → Bool → Bool
  | .and, a, b => a && b
  | .or, a, b => a || b
  | .imp, a, b => !a || b

/-- Complete output equivalence. A false frozen node disables the connective;
    its children may still be constrained by their own separate query nodes. -/
def FrozenGate (op : Operation) (mask output left right : Bool) : Prop :=
  output = (mask && gateValue op left right)

theorem enabled_gate_full_equivalence (op : Operation) (output left right : Bool) :
    FrozenGate op true output left right ↔ output = gateValue op left right := by
  simp [FrozenGate]

theorem disabled_gate_leaves_inputs_free (op : Operation) (output left right : Bool) :
    FrozenGate op false output left right ↔ output = false := by
  simp [FrozenGate]

theorem disabled_implication_must_not_be_enforced :
    FrozenGate .imp false false false false ∧
      false ≠ gateValue .imp false false := by
  simp [FrozenGate, gateValue]

/-- Finite relation table for one gate, including all eight possible rows. -/
def gateRows (op : Operation) (mask : Bool) : List (Bool × Bool × Bool) :=
  ([false, true].flatMap fun output => [false, true].flatMap fun left =>
    [false, true].map fun right => (output, left, right)).filter fun row =>
      row.1 == (mask && gateValue op row.2.1 row.2.2)

theorem gate_rows_exact (op : Operation) (mask output left right : Bool) :
    (output, left, right) ∈ gateRows op mask ↔ FrozenGate op mask output left right := by
  cases op <;> cases mask <;> cases output <;> cases left <;> cases right <;>
    simp [FrozenGate, gateValue, gateRows]

def gateConstraint (op : Operation) (mask : Bool) (output left right : α) : Constraint α where
  arity := 3
  slot := fun i => if i.val = 0 then output else if i.val = 1 then left else right
  relation := fun row => (row 0, row 1, row 2) ∈ gateRows op mask

theorem gate_constraint_exact (op : Operation) (mask : Bool) (output left right : α)
    (v : Valuation α) :
    Holds (gateConstraint op mask output left right) v ↔
      FrozenGate op mask (v output) (v left) (v right) := by
  simp [Holds, gateConstraint, gate_rows_exact]

theorem full_and_equivalence_propagates_backwards (output left right : Bool)
    (valid : FrozenGate .and true output left right) (asserted : output = true) :
    left = true ∧ right = true := by
  cases left <;> cases right <;> simp_all [FrozenGate, gateValue]

/-- Force atoms absent from the frozen candidate false; atoms present in it
    retain both possible tested-world values. -/
def subsetDomains (candidate : Valuation α) : Domains α := fun a b =>
  b = true → candidate a = true

def StrictDrop (candidate tested : Valuation α) : Prop :=
  ∃ a, candidate a = true ∧ tested a = false

theorem subset_domains_exact (candidate tested : Valuation α) :
    Fits (subsetDomains candidate) tested ↔
      Sub (QueryCompaction.asAtoms tested) (QueryCompaction.asAtoms candidate) := by
  rfl

theorem strict_drop_exact (candidate tested : Valuation α) :
    StrictDrop candidate tested ↔
      ¬ Sub (QueryCompaction.asAtoms candidate) (QueryCompaction.asAtoms tested) := by
  classical
  constructor
  · rintro ⟨a, inside, outside⟩ h
    have impossible := h a inside
    simp [QueryCompaction.asAtoms, outside] at impossible
  · intro notSub
    apply Classical.byContradiction
    intro noDrop
    apply notSub
    intro a inside
    cases truth : tested a with
    | false => exact False.elim (noDrop ⟨a, inside, truth⟩)
    | true => exact truth

theorem proper_subset_domains_exact (candidate tested : Valuation α) :
    Fits (subsetDomains candidate) tested ∧ StrictDrop candidate tested ↔
      Ferraris.ProperSub (QueryCompaction.asAtoms tested) (QueryCompaction.asAtoms candidate) := by
  rw [subset_domains_exact, strict_drop_exact]
  rfl

theorem empty_candidate_has_no_proper_tested_world (tested : Valuation α) :
    ¬ (Fits (subsetDomains (fun _ : α => false)) tested ∧
      StrictDrop (fun _ : α => false) tested) := by
  rintro ⟨_, a, impossible, _⟩
  cases impossible

/-- Only the original frozen reduct and strict subset relation occur here.
    Candidate-only objective restrictions are deliberately not an argument. -/
def InnerQuery [DecidableEq α] (candidate tested : Valuation α)
    (theory : Ferraris.Theory α) : Prop :=
  StrictDrop candidate tested ∧ QueryCompaction.QueryModels candidate tested theory

theorem inner_query_exact [DecidableEq α] (candidate tested : Valuation α)
    (theory : Ferraris.Theory α) :
    Fits (subsetDomains candidate) tested ∧ InnerQuery candidate tested theory ↔
      Ferraris.ProperSub (QueryCompaction.asAtoms tested) (QueryCompaction.asAtoms candidate) ∧
      Ferraris.Models (QueryCompaction.asAtoms tested)
        (Ferraris.ReductTheory (QueryCompaction.asAtoms candidate) theory) := by
  rw [InnerQuery, ← and_assoc, proper_subset_domains_exact,
    QueryCompaction.query_models_iff_reduct]

/-- A complete local-query encoding over semantic valuations is an explicit
    premise. Backends with auxiliary/node variables use the extension theorem
    below instead of counting auxiliaries in the subset relation. -/
theorem propagated_conflict_establishes_stability [DecidableEq α]
    (candidate : Valuation α) (theory : Ferraris.Theory α)
    (original : Ferraris.Models (QueryCompaction.asAtoms candidate) theory)
    (cs : List (Constraint α))
    (encoding : ∀ tested, Fits (subsetDomains candidate) tested →
      (Models cs tested ↔ InnerQuery candidate tested theory))
    (n : Nat) (a : α) (empty : ∀ b, ¬ sweeps cs n (subsetDomains candidate) a b) :
    Ferraris.Stable (QueryCompaction.asAtoms candidate) theory := by
  apply (QueryCompaction.stable_iff_compacted_queries candidate theory).mpr
  refine ⟨original, ?_⟩
  rintro ⟨tested, proper, valid⟩
  have fits : Fits (subsetDomains candidate) tested :=
    (subset_domains_exact candidate tested).mpr proper.1
  have dropped := (strict_drop_exact candidate tested).mpr proper.2
  exact empty_domain_refutes cs n (subsetDomains candidate) a empty
    ⟨tested, fits, (encoding tested fits).mpr ⟨dropped, valid⟩⟩

/-- GPU node variables may live in a different universe from semantic atoms.
    Every real countermodel must have a fitting encoded extension, including
    the correct semantic projection. This explicit compiler-coverage premise
    prevents auxiliary values from changing proper-subset minimality. -/
theorem propagated_conflict_with_auxiliaries [DecidableEq α] {β : Type v}
    (candidate : Valuation α) (theory : Ferraris.Theory α)
    (original : Ferraris.Models (QueryCompaction.asAtoms candidate) theory)
    (cs : List (Constraint β)) (initial : Domains β)
    (project : Valuation β → Valuation α)
    (coverage : ∀ tested, Fits (subsetDomains candidate) tested →
      InnerQuery candidate tested theory →
      ∃ encoded, Fits initial encoded ∧ Models cs encoded ∧ project encoded = tested)
    (n : Nat) (a : β) (empty : ∀ b, ¬ sweeps cs n initial a b) :
    Ferraris.Stable (QueryCompaction.asAtoms candidate) theory := by
  apply (QueryCompaction.stable_iff_compacted_queries candidate theory).mpr
  refine ⟨original, ?_⟩
  rintro ⟨tested, proper, valid⟩
  have fits := (subset_domains_exact candidate tested).mpr proper.1
  have dropped := (strict_drop_exact candidate tested).mpr proper.2
  obtain ⟨encoded, encodedFits, encodedValid, _⟩ := coverage tested fits ⟨dropped, valid⟩
  exact empty_domain_refutes cs n initial a empty ⟨encoded, encodedFits, encodedValid⟩

/-- Two binary constraints suffice to expose the incompleteness of local
    relation projection: equality and disequality each support both values. -/
def parityConstraint (equal : Bool) : Constraint Bool where
  arity := 2
  slot := fun i => if i.val = 0 then false else true
  relation := fun row => if equal then row 0 = row 1 else row 0 ≠ row 1

theorem parity_projection_is_quiescent (equal : Bool) :
    narrow (parityConstraint equal) full = full := by
  funext a b
  apply propext
  constructor
  · intro _; trivial
  · intro _
    let v : Valuation Bool := fun q => if equal then b else if q = a then b else !b
    have fits : Fits full v := fun _ => trivial
    have valid : Holds (parityConstraint equal) v := by
      cases equal <;> cases a <;> cases b <;> simp [Holds, parityConstraint, v]
    have atA : v a = b := by cases equal <;> simp [v]
    simpa [atA] using narrow_retains_completion (parityConstraint equal) full v fits valid a

/-- A fixed point with every value still available can be globally UNSAT.
    Therefore quiet nonempty domains are residual, not a model or stability proof. -/
theorem quiescent_unknown_can_be_unsatisfiable :
    sweep [parityConstraint true, parityConstraint false] full = full ∧
      (∀ a : Bool, full a false ∧ full a true) ∧
      ¬ ∃ v, Fits full v ∧ Models [parityConstraint true, parityConstraint false] v := by
  refine ⟨?_, fun _ => ⟨True.intro, True.intro⟩, ?_⟩
  · simp [sweep, parity_projection_is_quiescent]
  · rintro ⟨v, _, valid⟩
    have equal := valid (parityConstraint true) (by simp)
    have different := valid (parityConstraint false) (by simp)
    simp [Holds, parityConstraint] at equal different
    exact different equal

/-- Classical modelhood alone is likewise insufficient: the self-implication
    has an empty proper-subset reduct model under its nonempty candidate. -/
theorem unsupported_self_implication_is_not_stable :
    Ferraris.Models (Full : Atoms Unit) [.imp (.atom ()) (.atom ())] ∧
      ¬ Ferraris.Stable (Full : Atoms Unit) [.imp (.atom ()) (.atom ())] := by
  classical
  constructor
  · intro F member
    simp at member
    subst F
    exact fun h => h
  · intro stable
    apply stable.2
    refine ⟨Empty, ⟨fun _ h => False.elim h, ?_⟩, ?_⟩
    · intro impossible
      exact impossible () True.intro
    · simp [Ferraris.Models, Ferraris.ReductTheory, Ferraris.Reduct,
        Ferraris.Satisfies, Full, Empty]

end Zetesis.Propagation
