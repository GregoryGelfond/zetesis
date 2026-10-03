import TickProjection

open Aeneas Aeneas.Std Aeneas.Data.Coinductive
open ZetesisExtract

/-!
# Choosing the stored control representative

The event interpretation reads handles, not the fixed observation bits retained
by the older external model. Those bits can therefore be set clear in the
representative used for projection, while live cancellation and expiry remain
returning observations. The handle and deadline configuration are preserved.
This does not clear a Rust cancellation token or constrain its observed values.
-/
namespace RuntimeControl

/-- Set only the two unused fixed-read bits in the logical representation. All
owner identities and the deadline's other fields remain unchanged. -/
def representative (control : zetesis_cpu.cancellation.Cancellation) :
    zetesis_cpu.cancellation.Cancellation :=
  { cancelled := { control.cancelled with value := ⟨false⟩ }
    deadline := control.deadline.map fun owner =>
      { owner with value := { owner.value with deadline :=
          { owner.value.deadline with value :=
            { owner.value.deadline.value with expired := ⟨false⟩ } } } } }

/-- The fixed interpretation sees clear bits in the chosen representative. The
runtime event interpretation continues to receive independent responses. -/
theorem fixed_clear (control : zetesis_cpu.cancellation.Cancellation) :
    EvaluatorControl.observation (representative control) = none := by
  cases configured : control.deadline <;>
    simp [representative, EvaluatorControl.observation, configured]

/-- The cancellation read continues to name the identical owner. -/
theorem cancellation_owner (control : zetesis_cpu.cancellation.Cancellation) :
    ContextEvents.cancelObject (representative control) = ContextEvents.cancelObject control := by
  rfl

/-- The entire eventful poll is unchanged by the representative choice, including
its optional deadline read and cancellation-before-expiry order. Thus clear
stored bits impose no restriction on the runtime read responses. -/
theorem poll_unchanged (control : zetesis_cpu.cancellation.Cancellation) :
    ContextEvents.poll (representative control) = ContextEvents.poll control := by
  cases configured : control.deadline with
  | none =>
      have absent : (representative control).deadline = none := by
        simp only [representative, configured, Option.map_none]
      rw [ContextEvents.poll_without_deadline _ absent,
        ContextEvents.poll_without_deadline _ configured, cancellation_owner]
  | some owner =>
      let represented := { owner with value := { owner.value with deadline :=
          { owner.value.deadline with value :=
            { owner.value.deadline.value with expired := ⟨false⟩ } } } }
      have present : (representative control).deadline = some represented := by
        simp only [representative, configured, Option.map_some, represented]
      rw [TickProjection.poll_with_deadline _ represented present,
        TickProjection.poll_with_deadline _ owner configured, cancellation_owner]

end RuntimeControl
