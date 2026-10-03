import PublicMembership
import InterpretationStorage
import InsertionBoundaryExample
import InterpretationConstruction
import OwnedMembership
import MembershipVerdicts
import RuntimeEffects
import MembershipSearch
import SelectedAtoms
import ScalarSubsetsExample
import SubsetSteps
import SubsetQuery
import ProgressExample
import TheorySatisfaction
import RootScanExample
import FixedLoopExample
import FrozenQueryExample
import AdmissionValidationExample
import AdmittedData
import TheoryConstructionExample

import ReservedStorage
import WorkInitialization
import FrozenBoundaryExample

-- Complete authored-theorem audit for this optional refinement package.
#print axioms Membership.mask_bit
#print axioms SubsetQuery.completed_admission
#print axioms SubsetQuery.completed_phases
#print axioms SubsetQuery.completed_reduct
#print axioms SubsetQuery.completed_subset_count
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
#print axioms EvaluationSpecification.masked_exact
#print axioms EvaluationSpecification.children_present
#print axioms EvaluationSpecification.values_length
#print axioms EvaluationSpecification.prefix_length
#print axioms EvaluationSpecification.values_append
#print axioms EvaluationSpecification.prefix_succ
#print axioms EvaluationSpecification.prefix_complete
#print axioms EvaluatorSetup.evaluate_from_empty
#print axioms EvaluatorSetup.initial_alignment
#print axioms EvaluationTrace.invocation_refines
#print axioms EvaluationTrace.trace_refines
#print axioms EvaluationTrace.completed_values
#print axioms EvaluationTrace.stopped_prefix_shorter
#print axioms EvaluationTrace.trace_exists
#print axioms EvaluationSemantics.node_value_exact
#print axioms EvaluationSemantics.original_values
#print axioms EvaluationSemantics.frozen_values
#print axioms EvaluationSemantics.frozen_values_correspond
#print axioms EvaluationSemantics.frozen_value_at
#print axioms ReductTrace.initial_invariant
#print axioms ReductTrace.completed_original_values
#print axioms ReductTrace.completed_reduct_values
#print axioms ReductTrace.completed_reduct_truth
#print axioms ReductTrace.completed_reduct_satisfaction
#print axioms FixedEvaluationLoop.calls_are_trace
#print axioms FixedEvaluationLoop.loop_unfold
#print axioms FixedEvaluationLoop.calls_execute
#print axioms FixedEvaluationLoop.calls_exist
#print axioms FixedEvaluationLoop.loop_refines
#print axioms FixedEvaluationLoop.evaluate_refines
#print axioms FixedEvaluationLoop.completed_values
#print axioms FixedReductEvaluation.completed_values
#print axioms FixedReductEvaluation.completed_satisfaction
#print axioms ChangingObservations.refreshed_trace_differs_from_fixed_loop
#print axioms ChangingObservations.not_every_refreshed_trace_executes
#print axioms FixedRootScan.passed_prepend
#print axioms FixedRootScan.report_prepend
#print axioms FixedRootScan.report_none_iff
#print axioms FixedRootScan.read_exact
#print axioms FixedRootScan.body_exhausted
#print axioms FixedRootScan.body_stopped
#print axioms FixedRootScan.body_tested
#print axioms FixedRootScan.loop_unfold
#print axioms FixedRootScan.loop_stopped
#print axioms FixedRootScan.loop_refines
#print axioms FixedRootScan.failed_root_from_start
#print axioms FixedRootScan.failed_root_refines
#print axioms FixedRootScan.returned_report
#print axioms FixedRootScan.completed_none_iff
#print axioms FixedRootScan.returned_work_bound
#print axioms RootSemantics.original_roots
#print axioms RootSemantics.reduct_roots
#print axioms TheorySatisfaction.completed_original
#print axioms TheorySatisfaction.completed_reduct
#print axioms RootScanExample.empty_roots_do_not_poll
#print axioms RootScanExample.first_failure_follows_root_order
#print axioms EvaluationAccounting.returned_receipt
#print axioms EvaluationAccounting.completed_work
#print axioms EvaluationAccounting.returned_work_bound
#print axioms FrozenQuery.represents_from_evaluation
#print axioms FrozenQuery.mask_coverage
#print axioms FrozenQuery.execution_returns
#print axioms FrozenQuery.execution_exists
#print axioms FrozenQuery.returned_execution
#print axioms FrozenQuery.execution_satisfaction
#print axioms FrozenQuery.completed_satisfaction
#print axioms FrozenQuery.execution_work_bound
#print axioms FrozenQuery.returned_work_bound
#print axioms FrozenQueryExample.singleton_evaluation
#print axioms FrozenQueryExample.singleton_scan
#print axioms FrozenQueryExample.root_scan_retains_prior_work
#print axioms FrozenQueryExample.represented_candidate_need_not_be_a_model
#print axioms FrozenQueryExample.successful_query_need_not_test_a_subset
#print axioms SelectedAtoms.selected_length
#print axioms SelectedAtoms.selected_ordered
#print axioms SelectedAtoms.selected_nodup
#print axioms SelectedAtoms.advance
#print axioms SelectedAtoms.packed_denotation
#print axioms SelectedAtoms.completed_selection
#print axioms SelectedAtoms.exhausted
#print axioms ScalarSubsets.interpretation_storage
#print axioms ScalarSubsets.word_operations
#print axioms ScalarSubsets.updates_preserve_other_bits
#print axioms ScalarSubsets.set_population
#print axioms ScalarSubsets.clear_population
#print axioms ScalarSubsetsExample.setting_at_the_boundary_preserves_neighbors
#print axioms ScalarSubsetsExample.clearing_at_the_boundary_preserves_neighbors
#print axioms SubsetSteps.selection_exhausted
#print axioms SubsetSteps.selection_refused
#print axioms SubsetSteps.carry_exhausted
#print axioms SubsetSteps.carry_refused
#print axioms FixedSelection.body_advances
#print axioms FixedSelection.loop_unfold
#print axioms FixedSelection.loop_refused
#print axioms FixedSelection.loop_refines
#print axioms FixedSelection.select_refines
#print axioms FixedSelection.returned_report
#print axioms FixedSelection.completed_selection
#print axioms FixedSelection.returned_work_bound
#print axioms FixedSelection.select_completes
#print axioms FixedSelection.completed_carrier
#print axioms SubsetCarry.body_set
#print axioms SubsetCarry.body_clear
#print axioms SubsetCarry.loop_unfold
#print axioms SubsetCarry.loop_refused
#print axioms SubsetCarry.loop_typed
#print axioms SubsetCarry.advance_typed
#print axioms SubsetCarry.loop_success
#print axioms SubsetCarry.advance_success
#print axioms SubsetQueryTotal.evaluation_control
#print axioms SubsetQueryTotal.query_refines
#print axioms SubsetQueryTotal.returned_receipt
#print axioms CountermodelSteps.exhausted
#print axioms CountermodelSteps.query_stopped
#print axioms CountermodelSteps.witness
#print axioms CountermodelSteps.carry_stopped
#print axioms CountermodelSteps.continued
#print axioms SearchRepresentation.stored
#print axioms SearchRepresentation.denotes
#print axioms SearchRepresentation.selected_values
#print axioms SearchRepresentation.proper
#print axioms CountermodelSemantics.frozen_covered
#print axioms CountermodelSemantics.query_meaning
#print axioms CountermodelSemantics.query_refutes
#print axioms CountermodelTrace.loop_unfold
#print axioms CountermodelTrace.calls_execute
#print axioms CountermodelTrace.entry_executes
#print axioms FixedCountermodelSearch.calls_refine
#print axioms MembershipSearch.empty_search
#print axioms MembershipSearch.completed_countermodel
#print axioms MembershipSearch.selected_countermodel
#print axioms MembershipSearch.completed_answer_set
#print axioms RuntimeOwnership.clone_exact
#print axioms RuntimeOwnership.clone_consistent
#print axioms RuntimeOwnership.ptr_eq_exact
#print axioms RuntimeOwnership.same_owner_value
#print axioms RuntimeOwnership.accepted_owner_value
#print axioms RuntimeOwnership.equal_values_distinct_owners
#print axioms OwnerChecks.identities_accept_iff
#print axioms OwnerChecks.identities_reject
#print axioms OwnerChecks.accepted_data
#print axioms OwnerChecks.accepted_fields
#print axioms OwnerChecks.theory_clone_exact
#print axioms OwnerChecks.theory_clone_consistent
#print axioms PackedSetup.exact_storage_readable
#print axioms PackedSetup.resize_zero
#print axioms PackedSetup.resized_empty
#print axioms PackedSetup.initialized_interpretation
#print axioms PackedSetup.initialized_membership
#print axioms MembershipVerdicts.original_failure
#print axioms MembershipVerdicts.returned_witness
#print axioms MembershipVerdicts.witness_excludes_membership
#print axioms OwnedMembership.completed
#print axioms RuntimeEffects.runs_bind
#print axioms RuntimeEffects.loop_continues
#print axioms RuntimeEffects.loop_finishes
#print axioms RuntimeEffects.embed_ok
#print axioms RuntimeEffects.embed_failure
#print axioms RuntimeEffects.embed_divergence
#print axioms RuntimeEffects.embed_bind
#print axioms RuntimeEffects.pure_loop_compatible
#print axioms RuntimeEffects.distinct_reads
#print axioms RuntimeEffects.reservation_refusal
#print axioms RuntimeEffects.reservation_success
#print axioms RuntimeEffects.loop_observes_change
#print axioms AdmissionValidation.verdict_accepts_iff
#print axioms AdmissionValidation.verdict_refuses_iff
#print axioms AdmissionValidation.refusal_ne_allocation
#print axioms AdmissionValidation.node_exact
#print axioms AdmissionValidation.valid_node_iff
#print axioms AdmissionValidation.node_accepts_iff
#print axioms AdmissionValidation.nodes_body_exhausted
#print axioms AdmissionValidation.nodes_body_present
#print axioms AdmissionValidation.nodes_loop_unfold
#print axioms AdmissionValidation.nodes_loop_exact
#print axioms AdmissionValidation.validate_nodes_exact
#print axioms AdmissionValidation.validate_nodes_accepts_iff
#print axioms AdmissionValidation.validate_nodes_refuses_iff
#print axioms AdmissionValidation.accepted_ordered
#print axioms AdmissionValidation.validate_nodes_never_allocation
#print axioms AdmissionValidation.root_exact
#print axioms AdmissionValidation.roots_body_exhausted
#print axioms AdmissionValidation.roots_body_present
#print axioms AdmissionValidation.roots_loop_unfold
#print axioms AdmissionValidation.roots_loop_exact
#print axioms AdmissionValidation.validate_roots_exact
#print axioms AdmissionValidation.validate_roots_accepts_iff
#print axioms AdmissionValidation.validate_roots_refuses_iff
#print axioms AdmissionValidation.validate_roots_never_allocation
#print axioms AdmissionValidation.accepted_structure
#print axioms AdmissionValidationExample.atom_refusal_precedes_later_edge
#print axioms AdmissionValidationExample.edge_refusal_precedes_later_atom
#print axioms AdmissionValidationExample.falsum_is_admitted
#print axioms AdmissionValidationExample.empty_nodes_are_admitted
#print axioms AdmissionValidationExample.empty_roots_are_admitted
#print axioms AdmissionValidationExample.repeated_stored_roots_are_admitted
#print axioms AdmissionValidationExample.missing_root_after_repetitions_is_refused
#print axioms AdmittedData.admit_exact
#print axioms AdmittedData.admit_accepts_iff
#print axioms AdmittedData.admit_refuses_iff
#print axioms AdmittedData.admit_never_allocation
#print axioms AdmittedData.admitted_word_counts
#print axioms AdmittedData.admitted_program_structure
#print axioms TheoryConstruction.refused
#print axioms TheoryConstruction.admitted_allocation
#print axioms TheoryConstruction.completed_phases
#print axioms TheoryConstruction.refused_iff
#print axioms TheoryConstruction.returned_value
#print axioms TheoryConstruction.returned_structure
#print axioms TheoryConstruction.returned_heap
#print axioms TheoryConstructionExample.refused_input_does_not_allocate
#print axioms TheoryConstructionExample.admitted_input_can_diverge
#print axioms TheoryConstructionExample.separate_invocations_can_return_distinct_owners

#print axioms ReservedStorage.reserve_exact

#print axioms ReservedStorage.completed_reservation

#print axioms ReservedStorage.returned_refusal

#print axioms ReservedStorage.returned_empty

#print axioms WorkInitialization.statistics_default

#print axioms FrozenConstruction.reservation_stopped

#print axioms FrozenConstruction.evaluated

#print axioms FrozenConstruction.completed_phases

#print axioms FrozenConstruction.completed_represents

#print axioms FrozenConstruction.completed_work

#print axioms FrozenConstruction.new_stopped

#print axioms FrozenConstruction.new_completed_phases

#print axioms FrozenConstruction.new_represents

#print axioms PublicFrozenQuery.wrong_owner

#print axioms PublicFrozenQuery.prepared

#print axioms PublicFrozenQuery.completed_phases

#print axioms PublicFrozenQuery.completed_satisfaction

#print axioms PublicFrozenQuery.constructed_satisfaction

#print axioms FrozenBoundaryExample.foreign_theory_is_refused_first

#print axioms FrozenBoundaryExample.empty_construction_still_polls

#print axioms UsizeCeiling.zero_divisor

#print axioms UsizeCeiling.word_count64

#print axioms InterpretationConstruction.phases_exact

#print axioms InterpretationConstruction.reservation_refused

#print axioms InterpretationConstruction.completed_phases

#print axioms InterpretationConstruction.completed_theory

#print axioms SliceInsertion.word_operations
#print axioms InsertionLoop.body_exhausted
#print axioms InsertionLoop.body_refused
#print axioms InsertionLoop.loop_unfold
#print axioms InsertionLoop.body_inserted
#print axioms InsertionLoop.loop_exact
#print axioms InsertionLoop.insert_exact
#print axioms InsertionLoop.completed_exact
#print axioms InsertionLoop.padding_preserved
#print axioms VectorInput.finite
#print axioms VectorInput.converted
#print axioms InterpretationStorage.counted_zero
#print axioms InterpretationStorage.completed_initialization
#print axioms InterpretationStorage.packed_queries
#print axioms InterpretationStorage.completed_pack
#print axioms InterpretationStorage.completed_vector
#print axioms InsertionBoundaryExample.invalid_atom_avoids_diverging_tail
#print axioms InsertionBoundaryExample.first_none_finishes_nonfused_input

#print axioms SearchFrame.frame_trans
#print axioms SearchFrame.returned_carry
#print axioms SearchFrame.calls_frame
#print axioms SearchFrame.returned_frame
#print axioms PublicCheckBoundary.wrong_owner
#print axioms PublicCheckBoundary.stopped
#print axioms PublicCheckBoundary.reservation_refused
#print axioms PublicCheckPhases.completed_phases
#print axioms PublicMembership.original_counterexample
#print axioms PublicMembership.searched_answer_set
#print axioms PublicMembership.searched_witness
#print axioms PublicMembership.completed_answer_set
#print axioms PublicMembership.completed_not_model
#print axioms PublicMembership.completed_nonminimal
