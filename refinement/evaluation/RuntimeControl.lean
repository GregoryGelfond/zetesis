import TickProjection

open Aeneas Aeneas.Std Aeneas.Data.Coinductive
open ZetesisExtract

/-!
# Choosing the stored control representative

The event interpretation reads handles, not the fixed observations retained by
the older external model. Its Boolean tokens can be set false and its optional
slot token to the captured active word in the representative used for projection.
Live cancellation, slot words and expiry remain independent returning observations.
Handle identities, slot presence, captured active word and deadline configuration
are preserved.
This does not clear a Rust cancellation token or constrain its observed values.
-/
namespace RuntimeControl

/-- Clear the fixed Boolean observations and make the fixed slot observation
match its captured active word. All handles and other control fields are retained. -/
def representative (control : zetesis_cpu.cancellation.Cancellation) :
    zetesis_cpu.cancellation.Cancellation :=
  { cancelled := { control.cancelled with value := ⟨false⟩ }
    deadline := control.deadline.map fun owner =>
      { owner with value := { owner.value with deadline :=
          { owner.value.deadline with value :=
            { owner.value.deadline.value with expired := ⟨false⟩ } } } }
    slot := control.slot.map fun member =>
      { member with state := { member.state with value := ⟨member.active⟩ } } }

/-- The fixed interpretation sees clear bits in the chosen representative. The
runtime event interpretation continues to receive independent responses. -/
theorem fixed_clear (control : zetesis_cpu.cancellation.Cancellation) :
    EvaluatorControl.observation (representative control) = none := by
  cases configured : control.deadline <;> cases membership : control.slot <;>
    simp [representative, EvaluatorControl.observation, configured, membership]

/-- The cancellation read continues to name the identical owner. -/
theorem cancellation_owner (control : zetesis_cpu.cancellation.Cancellation) :
    ContextEvents.cancelObject (representative control) = ContextEvents.cancelObject control := by
  rfl

/-- The eventful poll retains its handles and captured active word, so replacing
unused fixed observations cannot constrain any runtime response or read order. -/
theorem poll_unchanged (control : zetesis_cpu.cancellation.Cancellation) :
    ContextEvents.poll (representative control) = ContextEvents.poll control := by
  have membershipRetained :
      ContextEvents.slotRead (representative control) = ContextEvents.slotRead control := by
    cases configured : control.slot <;>
      simp only [ContextEvents.slotRead, representative, configured, Option.map_none, Option.map_some]
  have deadlineRetained :
      ContextEvents.deadlineRead (representative control) = ContextEvents.deadlineRead control := by
    cases configured : control.deadline <;>
      simp only [ContextEvents.deadlineRead, representative, configured, Option.map_none, Option.map_some]
  rw [ContextEvents.poll_reads, ContextEvents.poll_reads, cancellation_owner,
    membershipRetained, deadlineRetained]

end RuntimeControl
