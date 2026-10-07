import Native.Funs

open Aeneas Aeneas.Std Result
open ZetesisNativeExtract

/-!
# Initial work accounting

The public membership checker, frozen-reduct construction and reduct queries
start separate budgets with zero statistics. This shared record describes each
start; the theorem checks their actual generated default operation.
-/
namespace NativeWorkInitialization

/-- Start the supplied budget and control with no work or subsets charged. -/
def initial (limits : oracle.Limits)
    (cancellation : zetesis_cpu.cancellation.Cancellation) : oracle.Work :=
  { limits, cancellation, statistics := { work := 0#u64, subsets := 0#u64 } }

/-- The generated default operation produces exactly zero statistics. -/
theorem statistics_default : oracle.Statistics.Insts.CoreDefaultDefault.default =
    ok ({ work := 0#u64, subsets := 0#u64 } : oracle.Statistics) := by
  rfl

end NativeWorkInitialization
