import QueryEventsProjection
import SearchEventsProjection
import EvaluationContextProjection
import RootContextProjection
import SelectionContextProjection
import CarryContextProjection
import PublicEventsProjection

open Aeneas Aeneas.Std Result
open ZetesisExtract RuntimeEffects

/-!
# Concrete operational projection

These compositions instantiate every phase contract with its checked generated-
context theorem. No assumed evaluator, root scan, selected-atom scan or subset
advance remains. Completed executions preserve the actual work and result;
semantic membership is then obtained from the existing fixed-checker proof.
-/
namespace RuntimeProjection

/-- A completed runtime reduct query is the actual generated query, with the
same output and work; its initial poll consumes an observation. -/
theorem completed_query (program : theory.Theory) (subset : theory.Interpretation)
    (frozen : Slice Bool) (input output : alloc.vec.Vec Bool) (before after : oracle.Work)
    (answer : Bool) (events : List Event)
    (clear : EvaluatorControl.observation before.cancellation = none)
    (run : Runs (ReferenceEvents.checkSubset program subset frozen input before)
      events (.Ok answer, output, after)) :
    oracle.check_subset program subset frozen input before = ok (.Ok answer, output, after) ∧
      after.cancellation = before.cancellation ∧ events ≠ [] := by
  apply QueryEventsProjection.completed
    (fun p c f i b o a e h r => EvaluationContextProjection.completed_evaluate p c f i b e o a h r)
    (fun p v b f a e h r => RootContextProjection.completed p v b a e f h r)
    program subset frozen input output before after answer events clear run

/-- A completed runtime proper-subset search is the actual generated search,
including its retained witness and exact work. This instantiates both the query
and carry correspondences and assumes neither semantic coverage nor a correct
inner oracle. -/
theorem completed_search (program : theory.Theory) (frozen : Slice Bool) (selected : Slice Usize)
    (subset next : theory.Interpretation) (input output : alloc.vec.Vec Bool)
    (before after : oracle.Work) (answer : Bool) (events : List Event)
    (clear : EvaluatorControl.observation before.cancellation = none)
    (run : Runs (ReferenceEvents.findCountermodel program frozen selected subset input before)
      events (.Ok answer, next, output, after)) :
    oracle.find_countermodel program frozen selected subset input before =
      ok (.Ok answer, next, output, after) ∧ after.cancellation = before.cancellation := by
  apply SearchEventsProjection.completed_search
    (fun p c f i b o a v e h r => completed_query p c f i o b a v e h r)
    (fun s c n b o k a e h r => CarryContextProjection.completed s c o n k b a e h r)
    program frozen selected subset next input output before after answer events clear run

/-- Every successful public runtime check returns precisely the generated
checker result under the logical reservation projection. All phase contracts
are discharged by the concrete context proofs above. No semantic verdict,
inner successful call or allocator success is assumed. -/
theorem completed_check (program : theory.Theory) (candidate : theory.Interpretation)
    (limits : oracle.Limits) (control : zetesis_cpu.cancellation.Cancellation)
    (decision : oracle.Check) (events : List Event)
    (clear : EvaluatorControl.observation control = none)
    (run : Runs (ReferenceEvents.check program candidate limits control) events (.Ok decision)) :
    @oracle.check ReservationEvents.fixed program candidate limits control = ok (.Ok decision) := by
  exact PublicEventsProjection.completed
    (fun p c f i b o a e h r => EvaluationContextProjection.completed_evaluate p c f i b e o a h r)
    (fun p v b f a e h r => RootContextProjection.completed p v b a e f h r)
    (fun p c i b o a e h r => SelectionContextProjection.completed p c i o b a e h r)
    (fun p f s c i b v n o a e h r => completed_search p f s c n i o b a v e h r)
    program candidate limits control decision events clear run

end RuntimeProjection
