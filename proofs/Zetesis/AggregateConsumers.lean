import Zetesis.FiniteValues

/-!
# Aggregate proposals with downstream data consumers

A finite instruction declares required logical slots and one produced slot.
Readiness describes an already chosen order: every read comes from the initial
relational environment or an earlier producer. List recursion decreases the
remaining instruction count. This is not a proof of the Rust topological sorter,
free-input extraction, local safety, or the value cursor's backtracking.

A completed aggregate carrier covers its actual result; its other proposals can
be unrealizable. Consuming a covered proposal preserves its value association.
Every resulting clause retains the original aggregate equality and a static
filter. The frozen laws use the same original formula under arbitrary M/J.
Coverage is an explicit premise, not a conclusion about stopped support rounds.
Machine arithmetic, undefined-value refusal, allocation/work limits and source
admission are outside these denotational laws.
-/

namespace Zetesis.AggregateConsumers
open Ferraris
universe u v

structure Instruction where
  required : List Nat
  produced : Nat

def available (initial : List Nat) : List Instruction → List Nat
  | [] => initial
  | step :: rest => available (step.produced :: initial) rest

def Ready (initial : List Nat) : List Instruction → Prop
  | [] => True
  | step :: rest => (∀ input ∈ step.required, input ∈ initial) ∧
      Ready (step.produced :: initial) rest

/-- Available slots are exactly the inherited slots and completed outputs. -/
theorem available_iff (slot : Nat) (initial : List Nat) (steps : List Instruction) :
    slot ∈ available initial steps ↔
      slot ∈ initial ∨ slot ∈ steps.map Instruction.produced := by
  induction steps generalizing initial with
  | nil => simp [available]
  | cons step rest induction =>
    rw [available, induction]
    simp only [List.mem_cons, List.map_cons]
    simp only [or_assoc, or_left_comm]

/-- A complete schedule separates into a ready prefix and a ready continuation. -/
theorem ready_split (initial : List Nat) (before suffix : List Instruction) :
    Ready initial (before ++ suffix) ↔
      Ready initial before ∧ Ready (available initial before) suffix := by
  induction before generalizing initial with
  | nil => simp [Ready, available]
  | cons step rest induction =>
    simp only [List.cons_append, Ready, available, induction]
    exact and_assoc.symm

/-- No future output or uninitialized slot can justify an instruction's read. -/
theorem scheduled_input_has_prior_producer (initial : List Nat)
    (before suffix : List Instruction) (step : Instruction) (input : Nat)
    (ordered : Ready initial (before ++ step :: suffix))
    (read : input ∈ step.required) :
    input ∈ initial ∨ input ∈ before.map Instruction.produced := by
  have continuation : Ready (available initial before) (step :: suffix) :=
    (ready_split initial before (step :: suffix)).mp ordered |>.2
  have present : input ∈ available initial before := continuation.1 input read
  exact (available_iff input initial before).mp present

/-- The produced head value remains paired with its aggregate proposal. -/
def substitutions {V : Type u} {W : Type v} (carrier : List V)
    (keep : V → Bool) (consume : V → W) : List (V × W) :=
  (carrier.filter keep).map (fun value => (value, consume value))

/-- A completed covering carrier cannot lose an accepted actual result. -/
theorem covered_consumer {V : Type u} {W : Type v} (carrier : List V)
    (keep : V → Bool) (consume : V → W) (actual : V)
    (covered : actual ∈ carrier) (accepted : keep actual = true) :
    (actual, consume actual) ∈ substitutions carrier keep consume := by
  have retained : actual ∈ carrier.filter keep := by
    simp [covered, accepted]
  exact List.mem_map.mpr ⟨actual, retained, rfl⟩

/-- Static filtering adds no semantic atom and keeps the aggregate formula. -/
def clause {A : Type u} (aggregate : Formula A) (accepted : Bool)
    (head : Formula A) : Formula A :=
  .imp (.conj aggregate (if accepted then .imp .bot .bot else .bot)) head

theorem original_clause {A : Type u} (M : Atoms A)
    (aggregate head : Formula A) (accepted : Bool) :
    Satisfies M (clause aggregate accepted head) ↔
      (Satisfies M aggregate ∧ accepted = true → Satisfies M head) := by
  cases accepted <;> simp [clause, Satisfies]

/-- A false static filter stays vacuous in every frozen context. -/
theorem false_filter_frozen {A : Type u} (M J : Atoms A)
    (aggregate head : Formula A) :
    Satisfies J (Reduct M (clause aggregate false head)) := by
  classical
  simp [clause, Reduct, Satisfies]

/-- Identical computed heads preserve the complete aggregate clause in any
    enclosing frozen formula; the equality guard is not replaced by success. -/
theorem frozen_consumer_identity {A : Type u} {V : Type v}
    (M J : Atoms A) (aggregate : Formula A) (accepted : Bool)
    (left right : V) (same : left = right) (resolve : V → Formula A)
    (context : Formula A → Formula A) :
    Satisfies J (Reduct M (context (clause aggregate accepted (resolve left)))) ↔
      Satisfies J (Reduct M (context (clause aggregate accepted (resolve right)))) := by
  subst right
  rfl

/-- Membership in a proposal list does not assert either equality or its head. -/
theorem proposal_does_not_assert_head :
    0 ∈ [0] ∧ Satisfies (fun _ : Unit => False)
      (clause .bot true (.atom ())) ∧
      ¬ Satisfies (fun _ : Unit => False) (.atom ()) := by
  simp [clause, Satisfies]

end Zetesis.AggregateConsumers
