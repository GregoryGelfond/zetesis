import CheckerContexts
import ContextEvents
import ReservationEvents

open Aeneas Aeneas.Std
open ZetesisExtract RuntimeEffects

/-!
# The reference checker under returning observations

These definitions compose the source-checked contexts with one returning-event
interpretation. They introduce no evaluator, subset algorithm or semantic
oracle: scalar operations remain embedded generated calls, and loop bodies,
branches and their order come from the retained contexts. The call graph follows
original evaluation, root checking, selected-atom construction and frozen-reduct
subset search. Each reached poll and reservation receives its own observation.

Source-context correspondence and runtime object/library contracts are explicit
trust boundaries. The projection theorems, rather than these definitions alone,
connect successful event executions to the verified fixed-observation checker.
-/
namespace ReferenceEvents

local instance : MonadLift Result Computation where
  monadLift := RuntimeEffects.embed

/-- Evaluate the stored nodes, with returning reads at their actual work ticks. -/
def evaluate :=
  CheckerContexts.evaluate (M := Computation)
    (CheckerContexts.evaluationLoop RuntimeEffects.loop
      (RuntimeContexts.evaluationContext ContextEvents.tick))

/-- Check asserted roots, retaining the first actual failed root. -/
def failedRoot :=
  CheckerContexts.rootScan (M := Computation)
    (CheckerContexts.rootLoop RuntimeEffects.loop
      (CheckerContexts.rootBody ContextEvents.tick))

/-- Select the candidate's actual set bits through the original atom scan. -/
def selectAtoms :=
  CheckerContexts.selectAtoms (M := Computation)
    (CheckerContexts.selectionLoop RuntimeEffects.loop
      (CheckerContexts.selectionBody ContextEvents.tick))

/-- Advance the packed subset through its original carry loop. -/
def advanceSubset :=
  CheckerContexts.advanceSubset (M := Computation)
    (CheckerContexts.carryLoop RuntimeEffects.loop
      (CheckerContexts.carryBody ContextEvents.tick))

/-- Poll and charge the subset query, then evaluate and check its frozen roots. -/
def checkSubset :=
  CheckerContexts.subsetQuery (M := Computation) ContextEvents.poll evaluate failedRoot

/-- Search the actual selected coordinates for a proper-subset reduct model. -/
def findCountermodel :=
  CheckerContexts.findCountermodel (M := Computation)
    (CheckerContexts.searchLoop RuntimeEffects.loop
      (CheckerContexts.searchBody checkSubset advanceSubset))

/-- The public reference checker with returning observations at all reached
poll/reservation sites. It preserves the generated owner and result branches. -/
def check :=
  CheckerContexts.check (M := Computation) ContextEvents.poll ReservationEvents.reserve
    evaluate failedRoot selectAtoms findCountermodel

/-- The owned-candidate wrapper retains the candidate only after an actual
successful public check and propagates a typed stop unchanged. -/
def checkInterpretation :=
  CheckerContexts.checkInterpretation (M := Computation) check

end ReferenceEvents
