import Evaluator.Funs

open Aeneas Aeneas.Std Result
open ZetesisExtract

/-!
# Initial work accounting

Public frozen-reduct construction and queries start separate budgets with zero
statistics. This shared record describes both starts; the theorem checks the
actual generated default operation used by those wrappers.
-/
namespace WorkInitialization

/-- Start the supplied budget and control with no work or subsets charged. -/
def initial (limits : oracle.Limits)
    (cancellation : zetesis_cpu.cancellation.Cancellation) : oracle.Work :=
  { limits, cancellation, statistics := { work := 0#u64, subsets := 0#u64 } }

/-- The generated default operation produces exactly zero statistics. -/
theorem statistics_default : oracle.Statistics.Insts.CoreDefaultDefault.default =
    ok ({ work := 0#u64, subsets := 0#u64 } : oracle.Statistics) := by
  rfl

end WorkInitialization
