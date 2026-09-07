import Zetesis.FerrarisMask

/-!
# Classical query compaction after freezing the reduct

Signed wires and an executable gate planner fold constants, identical literals
and complementary literals. The remaining gate has all three Tseitin clauses,
which determine its output uniquely. A recursive query compactor uses this same
planner. Its truth preservation is connected to a concrete original-formula
truth mask computed before compaction, and then to Ferraris subset minimality.

These are classical query transformations. They do not rewrite the original
Ferraris theory or assert strong equivalence of an original formula and its
classically compacted version. Gate-local unique extension is proved; the Rust
DAG/CNF allocator, fresh-index generation, shared-node map, clause traversal,
SAT decisions, limits and blocking clauses are not refined by this module.
-/

namespace Zetesis.QueryCompaction

universe u
variable {α : Type u}

/-- A signed classical query variable. -/
structure Literal (α : Type u) where
  atom : α
  positive : Bool
deriving DecidableEq

def Literal.negated (literal : Literal α) : Literal α :=
  ⟨literal.atom, !literal.positive⟩

def Literal.eval (valuation : α → Bool) (literal : Literal α) : Bool :=
  if literal.positive then valuation literal.atom else !(valuation literal.atom)

theorem literal_negated_eval (valuation : α → Bool) (literal : Literal α) :
    literal.negated.eval valuation = !(literal.eval valuation) := by
  cases literal with
  | mk atom positive => cases positive <;> simp [Literal.negated, Literal.eval]

/-- A compacted query node is either a Boolean constant or a signed variable. -/
inductive Wire (α : Type u) where
  | constant : Bool → Wire α
  | literal : Literal α → Wire α
deriving DecidableEq

def Wire.eval (valuation : α → Bool) : Wire α → Bool
  | .constant value => value
  | .literal input => input.eval valuation

def Wire.negated : Wire α → Wire α
  | .constant value => .constant (!value)
  | .literal input => .literal input.negated

theorem wire_negated_eval (valuation : α → Bool) (wire : Wire α) :
    wire.negated.eval valuation = !(wire.eval valuation) := by
  cases wire <;> simp [Wire.negated, Wire.eval, literal_negated_eval]

/-- False selects AND; true selects OR, matching the native gate interface. -/
def gateValue (disjunction left right : Bool) : Bool :=
  if disjunction then left || right else left && right

/-- A gate is either an alias/constant or one gate still needing a fresh output. -/
inductive GatePlan (α : Type u) where
  | alias : Wire α → GatePlan α
  | fresh : Bool → Literal α → Literal α → GatePlan α

def GatePlan.eval (valuation : α → Bool) : GatePlan α → Bool
  | .alias wire => wire.eval valuation
  | .fresh disjunction left right =>
      gateValue disjunction (left.eval valuation) (right.eval valuation)

/-- Executable constant, identity, idempotence and complement compaction. -/
def compactGate [DecidableEq α] (disjunction : Bool) : Wire α → Wire α → GatePlan α
  | .constant a, .constant b => .alias (.constant (gateValue disjunction a b))
  | .constant value, other =>
      .alias (if value = disjunction then .constant value else other)
  | other, .constant value =>
      .alias (if value = disjunction then .constant value else other)
  | .literal a, .literal b =>
      if a = b then .alias (.literal a)
      else if a = b.negated then .alias (.constant disjunction)
      else .fresh disjunction a b

theorem compact_gate_eval [DecidableEq α] (valuation : α → Bool)
    (disjunction : Bool) (left right : Wire α) :
    (compactGate disjunction left right).eval valuation =
      gateValue disjunction (left.eval valuation) (right.eval valuation) := by
  cases left with
  | constant a =>
    cases right with
    | constant b => rfl
    | literal b =>
      cases a <;> cases disjunction <;>
        simp [compactGate, GatePlan.eval, Wire.eval, gateValue]
  | literal a =>
    cases right with
    | constant b =>
      cases b <;> cases disjunction <;>
        simp [compactGate, GatePlan.eval, Wire.eval, gateValue]
    | literal b =>
      by_cases same : a = b
      · subst b
        cases disjunction <;> simp [compactGate, GatePlan.eval, Wire.eval, gateValue]
      · by_cases complement : a = b.negated
        · subst a
          cases disjunction <;> cases truth : b.eval valuation <;>
            simp [compactGate, same, GatePlan.eval, Wire.eval, gateValue,
              literal_negated_eval, truth]
        · simp [compactGate, same, complement, GatePlan.eval, Wire.eval]

/-- All three clauses for an exact AND output, expressed as their Boolean truth. -/
def andClauses (output left right : Bool) : Bool :=
  (!output || left) && (!output || right) && (output || !left || !right)

/-- All three clauses for an exact OR output, expressed as their Boolean truth. -/
def orClauses (output left right : Bool) : Bool :=
  (output || !left) && (output || !right) && (!output || left || right)

def gateClauses (disjunction output left right : Bool) : Bool :=
  if disjunction then orClauses output left right else andClauses output left right

theorem and_clauses_exact (output left right : Bool) :
    andClauses output left right = true ↔ output = (left && right) := by
  cases output <;> cases left <;> cases right <;> decide

theorem or_clauses_exact (output left right : Bool) :
    orClauses output left right = true ↔ output = (left || right) := by
  cases output <;> cases left <;> cases right <;> decide

theorem gate_clauses_exact (disjunction output left right : Bool) :
    gateClauses disjunction output left right = true ↔
      output = gateValue disjunction left right := by
  cases disjunction <;> simp [gateClauses, gateValue, and_clauses_exact, or_clauses_exact]

/-- Given input truth values, the unconstrained fresh Boolean output has exactly
    one extension satisfying the full gate encoding. -/
theorem gate_clauses_unique_extension (disjunction left right : Bool) :
    ∃ output, gateClauses disjunction output left right = true ∧
      ∀ other, gateClauses disjunction other left right = true → other = output := by
  refine ⟨gateValue disjunction left right, ?_, ?_⟩
  · exact (gate_clauses_exact disjunction _ left right).mpr rfl
  · intro output valid
    exact (gate_clauses_exact disjunction output left right).mp valid

/-- An OR query can use the negated output of an AND of negated inputs.
    This is a classical wire encoding, applied only after the original mask. -/
theorem canonical_and_gate_exact (disjunction output left right : Bool) :
    andClauses output (if disjunction then !left else left)
      (if disjunction then !right else right) = true ↔
    (if disjunction then !output else output) = gateValue disjunction left right := by
  cases disjunction <;> cases output <;> cases left <;> cases right <;> decide

/-- Sorting two signed input wires does not change the complete AND encoding. -/
theorem and_clauses_commute (output left right : Bool) :
    andClauses output left right = andClauses output right left := by
  cases output <;> cases left <;> cases right <;> decide

/-- Repeated complete gates with the same (possibly commuted) signed inputs
    force equal outputs, justifying reuse of one existing auxiliary wire.
    This does not prove the Rust lookup table or fresh-variable allocation. -/
theorem repeated_and_outputs_equal (first second left right : Bool)
    (one : andClauses first left right = true)
    (two : andClauses second right left = true) : first = second := by
  rw [and_clauses_commute] at two
  exact ((and_clauses_exact first left right).mp one).trans
    ((and_clauses_exact second left right).mp two).symm

/-- An alias directly determines output truth; a remaining fresh gate uses CNF. -/
def GatePlan.realizes (valuation : α → Bool) (output : Bool) : GatePlan α → Prop
  | .alias wire => output = wire.eval valuation
  | .fresh disjunction left right =>
      gateClauses disjunction output (left.eval valuation) (right.eval valuation) = true

theorem compacted_gate_encoding_exact [DecidableEq α] (valuation : α → Bool)
    (disjunction : Bool) (left right : Wire α) (output : Bool) :
    (compactGate disjunction left right).realizes valuation output ↔
      output = gateValue disjunction (left.eval valuation) (right.eval valuation) := by
  have plan : ∀ (result : GatePlan α), result.realizes valuation output ↔
      output = result.eval valuation := by
    intro result
    cases result <;> simp [GatePlan.realizes, GatePlan.eval, gate_clauses_exact]
  rw [plan, compact_gate_eval]

/-- A classical query tree; signed query wires are separate from ASP negation. -/
inductive Query (α : Type u) where
  | wire : Wire α → Query α
  | negated : Query α → Query α
  | gate : Bool → Query α → Query α → Query α

def Query.eval (valuation : α → Bool) : Query α → Bool
  | .wire input => input.eval valuation
  | .negated query => !(query.eval valuation)
  | .gate disjunction left right =>
      gateValue disjunction (left.eval valuation) (right.eval valuation)

def planQuery : GatePlan α → Query α
  | .alias wire => .wire wire
  | .fresh disjunction left right => .gate disjunction (.wire (.literal left)) (.wire (.literal right))

theorem plan_query_eval (valuation : α → Bool) (plan : GatePlan α) :
    (planQuery plan).eval valuation = plan.eval valuation := by
  cases plan <;> rfl

def smartGate [DecidableEq α] (disjunction : Bool) : Query α → Query α → Query α
  | .wire left, .wire right => planQuery (compactGate disjunction left right)
  | left, right => .gate disjunction left right

theorem smart_gate_eval [DecidableEq α] (valuation : α → Bool) (disjunction : Bool)
    (left right : Query α) :
    (smartGate disjunction left right).eval valuation =
      gateValue disjunction (left.eval valuation) (right.eval valuation) := by
  cases left <;> cases right <;>
    simp [smartGate, Query.eval, plan_query_eval, compact_gate_eval]

def smartNegated : Query α → Query α
  | .wire wire => .wire wire.negated
  | query => .negated query

theorem smart_negated_eval (valuation : α → Bool) (query : Query α) :
    (smartNegated query).eval valuation = !(query.eval valuation) := by
  cases query <;> simp [smartNegated, Query.eval, wire_negated_eval]

/-- Recursively compact a classical query using only the proved wire planner. -/
def compact [DecidableEq α] : Query α → Query α
  | .wire wire => .wire wire
  | .negated query => smartNegated (compact query)
  | .gate disjunction left right => smartGate disjunction (compact left) (compact right)

theorem compact_eval [DecidableEq α] (valuation : α → Bool) (query : Query α) :
    (compact query).eval valuation = query.eval valuation := by
  induction query with
  | wire => rfl
  | negated query ih => simp [compact, smart_negated_eval, Query.eval, ih]
  | gate disjunction left right ihLeft ihRight =>
    simp [compact, smart_gate_eval, Query.eval, ihLeft, ihRight]

/-- Interpret a classical Boolean assignment as a predicate set of atoms. -/
def asAtoms (valuation : α → Bool) : Atoms α := fun atom => valuation atom = true

/-- The original formula's truth is computed before query compaction. -/
def formulaTruth (valuation : α → Bool) : Ferraris.Formula α → Bool
  | .atom atom => valuation atom
  | .bot => false
  | .conj left right => formulaTruth valuation left && formulaTruth valuation right
  | .disj left right => formulaTruth valuation left || formulaTruth valuation right
  | .imp left right => !(formulaTruth valuation left) || formulaTruth valuation right

theorem boolean_implication_iff (left right : Bool) :
    (!left || right) = true ↔ (left = true → right = true) := by
  cases left <;> cases right <;> decide

theorem formula_truth_iff (valuation : α → Bool) (formula : Ferraris.Formula α) :
    formulaTruth valuation formula = true ↔ Ferraris.Satisfies (asAtoms valuation) formula := by
  induction formula with
  | atom => rfl
  | bot => simp [formulaTruth, Ferraris.Satisfies]
  | conj left right ihLeft ihRight =>
    simp [formulaTruth, Ferraris.Satisfies, ihLeft, ihRight]
  | disj left right ihLeft ihRight =>
    simp [formulaTruth, Ferraris.Satisfies, ihLeft, ihRight]
  | imp left right ihLeft ihRight =>
    change ((!formulaTruth valuation left || formulaTruth valuation right) = true) ↔ _
    rw [boolean_implication_iff, ihLeft, ihRight]
    rfl

/-- Construct a query with the supplied frozen mask; never recompute it in J. -/
def maskedQuery (mask : Ferraris.Formula α → Bool) : Ferraris.Formula α → Query α
  | .atom atom => if mask (.atom atom) then .wire (.literal ⟨atom, true⟩) else .wire (.constant false)
  | .bot => .wire (.constant false)
  | .conj left right => if mask (.conj left right)
      then .gate false (maskedQuery mask left) (maskedQuery mask right) else .wire (.constant false)
  | .disj left right => if mask (.disj left right)
      then .gate true (maskedQuery mask left) (maskedQuery mask right) else .wire (.constant false)
  | .imp left right => if mask (.imp left right)
      then .gate true (.negated (maskedQuery mask left)) (maskedQuery mask right) else .wire (.constant false)

theorem masked_query_eval_iff (mask : Ferraris.Formula α → Bool) (valuation : α → Bool)
    (formula : Ferraris.Formula α) :
    (maskedQuery mask formula).eval valuation = true ↔
      Ferraris.MaskedEval (fun F => mask F = true) (asAtoms valuation) formula := by
  induction formula with
  | atom atom =>
    cases truth : mask (.atom atom) <;>
      simp [maskedQuery, truth, Query.eval, Wire.eval, Literal.eval, Ferraris.MaskedEval, asAtoms]
  | bot => simp [maskedQuery, Query.eval, Wire.eval, Ferraris.MaskedEval]
  | conj left right ihLeft ihRight =>
    cases truth : mask (.conj left right) <;>
      simp [maskedQuery, truth, Query.eval, Wire.eval, gateValue, Ferraris.MaskedEval, ihLeft, ihRight]
  | disj left right ihLeft ihRight =>
    cases truth : mask (.disj left right) <;>
      simp [maskedQuery, truth, Query.eval, Wire.eval, gateValue, Ferraris.MaskedEval, ihLeft, ihRight]
  | imp left right ihLeft ihRight =>
    cases truth : mask (.imp left right) with
    | false => simp [maskedQuery, truth, Query.eval, Wire.eval, Ferraris.MaskedEval]
    | true =>
      simp only [maskedQuery, truth, ↓reduceIte, Query.eval,
        gateValue, Ferraris.MaskedEval, true_and]
      rw [boolean_implication_iff, ihLeft, ihRight]

/-- Freeze an original candidate's mask, then compact only its classical query. -/
def reductQuery [DecidableEq α] (candidate : α → Bool) (formula : Ferraris.Formula α) : Query α :=
  compact (maskedQuery (formulaTruth candidate) formula)

theorem reduct_query_eval_iff [DecidableEq α] (candidate tested : α → Bool)
    (formula : Ferraris.Formula α) :
    (reductQuery candidate formula).eval tested = true ↔
      Ferraris.Satisfies (asAtoms tested) (Ferraris.Reduct (asAtoms candidate) formula) := by
  rw [reductQuery, compact_eval, masked_query_eval_iff]
  exact Ferraris.masked_eval_iff_reduct (asAtoms candidate) (asAtoms tested)
    (fun F => formulaTruth candidate F = true) (formula_truth_iff candidate) formula

/-- Assert compacted queries for every original theory root. -/
def QueryModels [DecidableEq α] (candidate tested : α → Bool) (theory : Ferraris.Theory α) : Prop :=
  ∀ F, F ∈ theory → (reductQuery candidate F).eval tested = true

theorem query_models_iff_reduct [DecidableEq α] (candidate tested : α → Bool)
    (theory : Ferraris.Theory α) :
    QueryModels candidate tested theory ↔
      Ferraris.Models (asAtoms tested) (Ferraris.ReductTheory (asAtoms candidate) theory) := by
  constructor
  · intro valid F member
    obtain ⟨original, inside, rfl⟩ := List.mem_map.mp member
    exact (reduct_query_eval_iff candidate tested original).mp (valid original inside)
  · intro valid F member
    exact (reduct_query_eval_iff candidate tested F).mpr
      (valid _ (List.mem_map.mpr ⟨F, member, rfl⟩))

/-- Every predicate interpretation has a classical Boolean representation. This
    denotational bridge does not require a finite or executable source carrier. -/
theorem exists_boolean_representation (atoms : Atoms α) :
    ∃ valuation, asAtoms valuation = atoms := by
  classical
  refine ⟨fun atom => decide (atoms atom), ?_⟩
  funext atom
  simp [asAtoms]

/-- Compaction leaves exactly the same proper-subset countermodels. Subset
    membership concerns original atoms, never gate-output auxiliaries. -/
theorem compacted_countermodels_iff [DecidableEq α] (candidate : α → Bool)
    (theory : Ferraris.Theory α) :
    (∃ tested, Ferraris.ProperSub (asAtoms tested) (asAtoms candidate) ∧
      QueryModels candidate tested theory) ↔
    ∃ J, Ferraris.ProperSub J (asAtoms candidate) ∧
      Ferraris.Models J (Ferraris.ReductTheory (asAtoms candidate) theory) := by
  constructor
  · rintro ⟨tested, proper, valid⟩
    exact ⟨asAtoms tested, proper, (query_models_iff_reduct candidate tested theory).mp valid⟩
  · rintro ⟨J, proper, valid⟩
    obtain ⟨tested, same⟩ := exists_boolean_representation J
    refine ⟨tested, ?_, ?_⟩
    · simpa [same] using proper
    · apply (query_models_iff_reduct candidate tested theory).mpr
      simpa [same] using valid

/-- Original modelhood and absence of a compacted frozen-query countermodel give
    exactly original Ferraris stability. No original-theory rewrite is needed. -/
theorem stable_iff_compacted_queries [DecidableEq α] (candidate : α → Bool)
    (theory : Ferraris.Theory α) :
    Ferraris.Stable (asAtoms candidate) theory ↔
      Ferraris.Models (asAtoms candidate) theory ∧
      ¬ ∃ tested, Ferraris.ProperSub (asAtoms tested) (asAtoms candidate) ∧
        QueryModels candidate tested theory := by
  rw [compacted_countermodels_iff]
  rfl

/-- Any chosen variable partitions satisfying assignments, irrespective of the
    heuristic selecting it. An already forced variable may have an empty branch. -/
theorem assignment_branch_partition (valid : (α → Bool) → Prop) (atom : α)
    (valuation : α → Bool) :
    valid valuation ↔
      (valuation atom = false ∧ valid valuation) ∨
      (valuation atom = true ∧ valid valuation) := by
  cases valuation atom <;> simp

/-- The two truth-value branches never share an assignment. -/
theorem assignment_branches_disjoint (valid : (α → Bool) → Prop) (atom : α) :
    ¬ ∃ valuation, (valuation atom = false ∧ valid valuation) ∧
      (valuation atom = true ∧ valid valuation) := by
  rintro ⟨valuation, ⟨negative, _⟩, ⟨positive, _⟩⟩
  simp [negative] at positive

/-- Refuting both complete branches refutes the parent query. The hypotheses
    require actual refutations; a bounded or interrupted branch cannot supply one. -/
theorem exhausted_branches_refute (valid : (α → Bool) → Prop) (atom : α)
    (negative : ∀ valuation, valuation atom = false → ¬ valid valuation)
    (positive : ∀ valuation, valuation atom = true → ¬ valid valuation) :
    ¬ ∃ valuation, valid valuation := by
  rintro ⟨valuation, accepted⟩
  rcases (assignment_branch_partition valid atom valuation).mp accepted with
    ⟨truth, valid⟩ | ⟨truth, valid⟩
  · exact negative valuation truth valid
  · exact positive valuation truth valid

end Zetesis.QueryCompaction
