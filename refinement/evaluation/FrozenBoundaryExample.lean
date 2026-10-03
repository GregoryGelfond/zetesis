import FrozenConstruction
import PublicFrozenQuery

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Public frozen-reduct refusal boundaries

These finite examples exercise actual generated public calls with an explicitly
supplied nonreturning reservation operation. They establish precedence even when
storage preparation would not return. The provider is a proof witness, not an
allocator implementation or a claim about runtime allocation behavior.
-/
namespace FrozenBoundaryExample

/-- An explicit reservation interpretation with no returned outcome. -/
@[reducible]
def divergingReservation : VectorReservation :=
  { reserve := fun {_} _ _ _ => div }

/-- Changing the owner changes only identity, not the empty theory's contents. -/
def emptyCandidate (owner : Nat) : theory.Interpretation := {
  theory := { owner, value := {
    atoms := 0#usize
    nodes := alloc.vec.Vec.new theory.Node
    roots := alloc.vec.Vec.new Usize } }
  words := alloc.vec.Vec.new U64 }

/-- No work or subsets are available. -/
def noBudget : oracle.Limits := { max_work := 0#u64, max_subsets := 0#u64 }

/-- The fixed cancellation observation is already true, with no deadline. -/
def cancelled : zetesis_cpu.cancellation.Cancellation := {
  cancelled := { owner := 2, value := { nextRead := true } }
  deadline := none }

/-- Equal theory contents do not override a different owner. The actual public
query returns WrongProgram despite cancellation and a reservation provider that
would diverge; neither later boundary replaces the identity refusal. -/
theorem foreign_theory_is_refused_first :
    @reduct.FrozenReduct.is_satisfied_by divergingReservation
      ⟨emptyCandidate 0, alloc.vec.Vec.new Bool⟩ (emptyCandidate 1) noBudget cancelled =
      ok (.Err .WrongProgram) := by
  exact PublicFrozenQuery.wrong_owner (reservation := divergingReservation)
    ⟨emptyCandidate 0, alloc.vec.Vec.new Bool⟩ (emptyCandidate 1) noBudget cancelled
    (by decide)

/-- Even an empty public construction polls before storage preparation. The
observed cancellation returns without relying on reservation completion or an
available work budget; empty input does not bypass the public control boundary. -/
theorem empty_construction_still_polls :
    @reduct.FrozenReduct.new divergingReservation (emptyCandidate 0) noBudget cancelled =
      ok (.Err .Cancelled) := by
  exact FrozenConstruction.new_stopped (reservation := divergingReservation)
    (emptyCandidate 0) noBudget cancelled .Cancelled rfl

end FrozenBoundaryExample
