import Zetesis.OperandArena

/-!
# Topological evaluation of finite operand groups

A finite formula table stores atoms, falsum, implication and arbitrary finite
conjunctions or disjunctions. Groups read checked spans from one immutable
operand arena. The original pass and a paired original/reduct pass compute the
truth of every decoded formula. Missing references return no result.

The paired pass computes each original mask bit before using it; mask agreement
is therefore a conclusion, not an input assumption. Its schedule is mathematical:
correspondence to the runtime's separate passes, inline pairs, machine bounds,
work limits and ownership is not established here. The formulas and reduct are
the existing Ferraris definitions, including arbitrary tested interpretations.
-/

namespace Zetesis.Refinement.OperandTable

universe u
variable {α : Type u}
open Ferraris TightEvaluation

/-- A formula layer refers only to earlier layers. Operand groups share the
same arena regardless of connective; implication keeps its two ordered sides. -/
inductive Node (α : Type u) where
  | atom (value : α)
  | bot
  | group (kind : OperandArena.Connective) (span : OperandArena.Span)
  | imp (left right : Nat)

/-- All references are present in the completed prefix. Group admission also
checks the complete arena range, including an empty span. -/
def Valid (arena : List Nat) (earlier : Nat) : Node α → Prop
  | .atom _ | .bot => True
  | .group _ span => OperandArena.Admitted arena earlier span
  | .imp left right => left < earlier ∧ right < earlier

/-- Construct a table by appending one layer whose children already exist. -/
inductive WellFormed (arena : List Nat) : List (Node α) → Prop where
  | nil : WellFormed arena []
  | snoc {table entry} : WellFormed arena table → Valid arena table.length entry →
      WellFormed arena (table ++ [entry])

/-- Decode a layer using its previously decoded formulas. Falsum totalizes
invalid references only in this specification; successful checked evaluation
never uses those defaults. -/
def decode (arena : List Nat) (earlier : List (Formula α)) : Node α → Formula α
  | .atom atom => .atom atom
  | .bot => .bot
  | .group kind span => OperandArena.meaning kind earlier arena span
  | .imp left right => .imp (earlier.getD left .bot) (earlier.getD right .bot)

/-- Decode the complete table in stored order, reusing previously decoded formulas. -/
def meanings (arena : List Nat) (table : List (Node α)) : List (Formula α) :=
  table.foldl (fun earlier entry => earlier ++ [decode arena earlier entry]) []

/-- Each stored layer contributes exactly one decoded formula. -/
theorem meanings_length (arena : List Nat) (table : List (Node α)) :
    (meanings arena table).length = table.length := by
  have prefixLength (remaining : List (Node α)) (earlier : List (Formula α)) :
      (remaining.foldl (fun decoded entry => decoded ++ [decode arena decoded entry])
        earlier).length = earlier.length + remaining.length := by
    induction remaining generalizing earlier with
    | nil => simp
    | cons entry rest inductionHypothesis =>
      simp only [List.foldl_cons, inductionHypothesis, List.length_append,
        List.length_cons, List.length_nil]
      omega
  simpa [meanings] using prefixLength table []

/-- Read every required child before returning a layer's Boolean value. -/
def read (candidate : α → Bool) (arena : List Nat) (earlier : List Bool) :
    Node α → Option Bool
  | .atom atom => some (candidate atom)
  | .bot => some false
  | .group kind span => OperandArena.evaluate kind earlier arena span
  | .imp left right => do return !(← earlier[left]?) || (← earlier[right]?)

/-- One checked original layer equals the truth of its decoded formula. -/
theorem read_original_exact (candidate : α → Bool) (arena : List Nat)
    (earlier : List (Formula α)) (entry : Node α)
    (valid : Valid arena earlier.length entry) :
    read candidate arena (earlier.map (formulaValue candidate)) entry =
      some (formulaValue candidate (decode arena earlier entry)) := by
  cases entry with
  | atom atom => rfl
  | bot => rfl
  | group kind span => exact OperandArena.original_exact kind earlier candidate arena span valid
  | imp left right =>
    simp only [Valid] at valid
    simp [read, decode, formulaValue, List.getElem?_map,
      List.getD_eq_getElem?_getD, List.getElem?_eq_getElem valid.1,
      List.getElem?_eq_getElem valid.2]

/-- Combining reduct child truths with the current original bit gives the
explicit reduct's truth. The tested interpretation is unrestricted. -/
theorem read_reduct_exact (outer tested : α → Bool) (arena : List Nat)
    (earlier : List (Formula α)) (entry : Node α)
    (valid : Valid arena earlier.length entry) :
    (read tested arena (earlier.map (fun child =>
        formulaValue tested (Reduct (interpretation outer) child))) entry).map
        (fun inner => inner && formulaValue outer (decode arena earlier entry)) =
      some (formulaValue tested (Reduct (interpretation outer)
        (decode arena earlier entry))) := by
  classical
  cases entry with
  | atom atom =>
    cases value : outer atom <;> simp [read, decode, formulaValue, Reduct, interpretation, value]
  | bot => rfl
  | group kind span =>
    change (OperandArena.evaluate kind _ arena span).map _ = _
    rw [OperandArena.reduct_exact kind earlier outer tested arena span valid]
    simp only [Option.map_some, decode]
    apply congrArg some
    by_cases holds : Satisfies (interpretation outer) (OperandArena.meaning kind earlier arena span)
    · have originalTrue : formulaValue outer
          (OperandArena.meaning kind earlier arena span) = true :=
        (formula_value_true outer _).mpr holds
      simp [originalTrue]
    · have falseReduct : Reduct (interpretation outer)
          (OperandArena.meaning kind earlier arena span) = .bot :=
        RuleFactorization.false_reduct _ _ holds
      simp [falseReduct, formulaValue]
  | imp left right =>
    simp only [Valid] at valid
    simp only [read, List.getElem?_map, List.getElem?_eq_getElem valid.1,
      List.getElem?_eq_getElem valid.2, Option.map_some,
      pure, decode, List.getD_eq_getElem?_getD, Option.getD_some]
    apply congrArg some
    apply Bool.eq_iff_iff.mpr
    simp only [Bool.and_eq_true, Bool.or_eq_true, Bool.not_eq_true',
      Bool.eq_false_iff, formula_value_true, RuleFactorization.reduct_imp]
    constructor
    · rintro ⟨alternative, originalTrue⟩
      exact ⟨originalTrue, fun leftTrue =>
        alternative.elim (fun absent => False.elim
          (absent ((formula_value_true tested _).mpr leftTrue))) id⟩
    · rintro ⟨originalTrue, implication⟩
      by_cases leftTrue : Satisfies (interpretation tested)
          (Reduct (interpretation outer) earlier[left])
      · exact ⟨Or.inr (implication leftTrue), originalTrue⟩
      · exact ⟨Or.inl (fun value => leftTrue ((formula_value_true tested _).mp value)),
          originalTrue⟩

/-- Append an original truth only after all its checked child reads succeed. -/
def originalStep (candidate : α → Bool) (arena : List Nat)
    (earlier : List Bool) (entry : Node α) : Option (List Bool) := do
  let value ← read candidate arena earlier entry
  return earlier ++ [value]

/-- A complete original pass; absence is distinct from a false result. -/
def original (candidate : α → Bool) (arena : List Nat)
    (table : List (Node α)) : Option (List Bool) :=
  table.foldlM (originalStep candidate arena) []

/-- Every admitted original pass completes with the exact decoded truth table.
The induction extends the already correct prefix by one admitted layer. -/
theorem original_exact (candidate : α → Bool) (arena : List Nat)
    (table : List (Node α)) (admitted : WellFormed arena table) :
    original candidate arena table =
      some ((meanings arena table).map (formulaValue candidate)) := by
  induction admitted with
  | nil => rfl
  | @snoc table entry admitted valid inductionHypothesis =>
    have prefixValid : Valid arena (meanings arena table).length entry := by
      simpa only [meanings_length] using valid
    have next : read candidate arena
        ((meanings arena table).map (formulaValue candidate)) entry =
        some (formulaValue candidate (decode arena (meanings arena table) entry)) :=
      read_original_exact candidate arena (meanings arena table) entry prefixValid
    simp only [original, List.foldlM_append] at inductionHypothesis ⊢
    rw [inductionHypothesis]
    simp only [List.foldlM_cons, List.foldlM_nil, bind_pure]
    change originalStep candidate arena _ entry = _
    rw [show meanings arena (table ++ [entry]) =
      meanings arena table ++ [decode arena (meanings arena table) entry] by
        simp [meanings, List.foldl_append]]
    simp [originalStep, next]

/-- The original and reduct prefixes belong to the same processed node table. -/
structure Truth where
  original : List Bool
  reduct : List Bool
  deriving DecidableEq

/-- Compute the current original bit before masking the tested layer. Nothing
is appended unless both checked reads finish. -/
def reductStep (outer tested : α → Bool) (arena : List Nat)
    (earlier : Truth) (entry : Node α) : Option Truth := do
  let mask ← read outer arena earlier.original entry
  let inner ← read tested arena earlier.reduct entry
  return ⟨earlier.original ++ [mask], earlier.reduct ++ [inner && mask]⟩

/-- Paired topological evaluation computes its own original mask. -/
def reduct (outer tested : α → Bool) (arena : List Nat)
    (table : List (Node α)) : Option Truth :=
  table.foldlM (reductStep outer tested arena) ⟨[], []⟩

/-- An admitted paired pass computes both original truth and exact frozen-reduct
truth. The mask premise of a single-layer law is discharged by the original read
at that layer; no agreement assumption about stored Boolean values remains. -/
theorem reduct_exact (outer tested : α → Bool) (arena : List Nat)
    (table : List (Node α)) (admitted : WellFormed arena table) :
    reduct outer tested arena table = some ⟨
      (meanings arena table).map (formulaValue outer),
      (meanings arena table).map (fun entry =>
        formulaValue tested (Reduct (interpretation outer) entry))⟩ := by
  induction admitted with
  | nil => rfl
  | @snoc table entry admitted valid inductionHypothesis =>
    have prefixValid : Valid arena (meanings arena table).length entry := by
      simpa only [meanings_length] using valid
    have nextOriginal : read outer arena
        ((meanings arena table).map (formulaValue outer)) entry =
        some (formulaValue outer (decode arena (meanings arena table) entry)) :=
      read_original_exact outer arena (meanings arena table) entry prefixValid
    have nextReduct : (read tested arena
        ((meanings arena table).map (fun child =>
          formulaValue tested (Reduct (interpretation outer) child))) entry).map
          (fun inner => inner && formulaValue outer (decode arena (meanings arena table) entry)) =
        some (formulaValue tested (Reduct (interpretation outer)
          (decode arena (meanings arena table) entry))) :=
      read_reduct_exact outer tested arena (meanings arena table) entry prefixValid
    simp only [reduct, List.foldlM_append] at inductionHypothesis ⊢
    rw [inductionHypothesis]
    simp only [List.foldlM_cons, List.foldlM_nil, bind_pure]
    change reductStep outer tested arena _ entry = _
    rw [show meanings arena (table ++ [entry]) =
      meanings arena table ++ [decode arena (meanings arena table) entry] by
        simp [meanings, List.foldl_append]]
    simp only [reductStep, nextOriginal, List.map_append,
      List.map_singleton]
    cases result : read tested arena
        ((meanings arena table).map (fun child =>
          formulaValue tested (Reduct (interpretation outer) child))) entry with
    | none => simp [result] at nextReduct
    | some value =>
      simp only [result, Option.map_some, Option.some.injEq] at nextReduct
      simp [nextReduct]

end Zetesis.Refinement.OperandTable
