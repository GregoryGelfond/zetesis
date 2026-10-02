import ProgressExample

-- Complete authored-theorem audit for this optional refinement package.
#print axioms Membership.mask_bit
#print axioms Membership.outside
#print axioms Membership.contains_refines
#print axioms EvaluatorIteration.slice_next_present
#print axioms EvaluatorIteration.slice_next_exhausted
#print axioms EvaluatorIteration.enumerate_next_present
#print axioms EvaluatorIteration.enumerate_next_exhausted
#print axioms EvaluatorIteration.vector_read
#print axioms EvaluatorIteration.vector_append
#print axioms EvaluatorControl.poll_exact
#print axioms EvaluatorControl.tick_stopped
#print axioms EvaluatorControl.tick_at_limit
#print axioms EvaluatorControl.tick_advances
#print axioms Evaluation.read_exact
#print axioms Evaluation.mask_exact
#print axioms Evaluation.masked_append
#print axioms Evaluation.step_after_tick
#print axioms Evaluation.step_stops
#print axioms Evaluation.exhausted_step
#print axioms EvaluationProgress.present_step
#print axioms EvaluationProgress.control_stops
#print axioms EvaluationProgress.work_limit_stops
#print axioms EvaluationProgressExample.false_node_advances
