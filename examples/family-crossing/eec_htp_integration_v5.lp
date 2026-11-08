% ============================================================
% eec_htp_integration_v5.lp
%
% Integrated Extended Event Calculus (EEC) + Hierarchical Task
% Planning (HTP) Adapter
%
% This rewrite preserves the original predicate interface while
% using descriptive variable names and documented argument roles.
%
% ============================================================
%
% PREDICATE SIGNATURES
%
% DOMAIN TYPES
%   primitive(PrimitiveAction)
%   compound(CompoundTask)
%   task(Task)
%   method(MethodName, CompoundTask)
%   fluent(StateFluent)
%   event(EventName)
%   timepoint(Time)
%   step(Time)
%
% EEC EVENT / STATE PREDICATES
%   happens(EventName, EventTime)
%   occurs(PrimitiveAction, OccurrenceTime)
%   holds_at(StateFluent, QueryTime)
%   holds(StateFluent, QueryTime)
%   released_at(StateFluent, QueryTime)
%
%   initiates(EventName, StateFluent, EventTime)
%   terminates(EventName, StateFluent, EventTime)
%   releases(EventName, StateFluent, EventTime)
%
%   trajectory(SourceFluent, EventTime, ResultFluent, Delay)
%   anti_trajectory(SourceFluent, EventTime, ResultFluent, Delay)
%
% EEC INTERVAL DIAGNOSTICS
%   clipped(IntervalStartTime, StateFluent, IntervalEndTime)
%   declipped(IntervalStartTime, StateFluent, IntervalEndTime)
%   stopped_in(IntervalStartTime, StateFluent, IntervalEndTime)
%   started_in(IntervalStartTime, StateFluent, IntervalEndTime)
%   released_at_between(IntervalStartTime, StateFluent, IntervalEndTime)
%   released_between(IntervalStartTime, StateFluent, IntervalEndTime)
%   released_in(IntervalStartTime, StateFluent, IntervalEndTime)
%
% STATIC DOMAIN INPUTS
%   add_effect(PrimitiveAction, StateFluent)
%   del_effect(PrimitiveAction, StateFluent)
%   precondition_static(PrimitiveAction, StateFluent)
%   negative_precondition_static(PrimitiveAction, StateFluent)
%   method_precondition(MethodName, StateFluent)
%   goal(StateFluent, GoalEvaluationTime)
%
% DERIVED EEC PRECONDITION PREDICATES
%   precondition(EventName, StateFluent, QueryTime)
%   negative_precondition(EventName, StateFluent, QueryTime)
%   precondition_satisfied(EventName, StateFluent, QueryTime)
%   negative_precondition_satisfied(EventName, StateFluent, QueryTime)
%   precondition_violated(EventName, StateFluent, QueryTime)
%   negative_precondition_violated(EventName, StateFluent, QueryTime)
%
% HTP EXTERNAL DOMAIN CONTRACT
%   selected(MethodName, CompoundTask, TaskStartTime)
%   applied(MethodName, MethodStartTime, MethodEndTime)
%   task_start(CompoundTask, TaskStartTime)
%   task_span(CompoundTask, TaskStartTime, TaskEndTime)
%   root_task(CompoundTask, RootTaskStartTime)
%   htp_eligible(PrimitiveAction, EligibleTime)
%
% EXECUTABILITY / PLAN STATUS
%   can_happen(EventName, CandidateTime)
%   cannot_happen(EventName, CandidateTime)
%   all_preconditions_met(EventName, CandidateTime)
%   eligible_but_blocked(EventName, CandidateTime)
%   outside_selected_decomposition(EventName, CandidateTime)
%   plan_valid
%   plan_invalid
%
% ============================================================
%
% PURPOSE
%
%   1. Provides Extended Event Calculus state-transition semantics.
%   2. Derives action precondition satisfaction and violation.
%   3. Bridges primitive HTP actions to EEC events.
%   4. Constrains can_happen/2 through htp_eligible/2 when an
%      external HTP domain activates external_htp_domain.
%
% EXTERNAL HTP CONTRACT
%
% When external_htp_domain. is supplied by a scenario:
%
%   htp_eligible(PrimitiveAction, EligibleTime)
%
% must be derived only when PrimitiveAction is a primitive leaf
% of the selected and applied decomposition tree at EligibleTime.
%
% The external HTP domain owns:
%
%   compound/1
%   primitive/1
%   method/2
%   subtask/3
%   selected/3
%   applied/3
%   task_start/2
%   task_span/3
%   htp_eligible/2
%   Mode A and Mode B occurrence generation
%
% ============================================================
% §1 DOMAIN-PREDICATE SUPPORT
% ============================================================

task(PrimitiveAction) :-
    primitive(PrimitiveAction).

task(CompoundTask) :-
    compound(CompoundTask).

% A scenario may derive timepoint/1 from step/1.  This adapter
% deliberately does not derive step/1 from timepoint/1 because
% that could extend successor-state reasoning past the declared
% planning horizon.
timepoint(Time) :-
    step(Time).

% ============================================================
% §2 EXTENDED EVENT CALCULUS CORE AXIOMS
% ============================================================

% ------------------------------------------------------------
% EC1: Clipping
%
% StateFluent is clipped in [IntervalStartTime, IntervalEndTime)
% when an event terminates it inside that interval.
% ------------------------------------------------------------

clipped(IntervalStartTime, StateFluent, IntervalEndTime) :-
    timepoint(EffectTime),
    timepoint(IntervalStartTime),
    timepoint(IntervalEndTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, EffectTime),
    terminates(EventName, StateFluent, EffectTime),
    IntervalStartTime <= EffectTime,
    EffectTime < IntervalEndTime.

% ------------------------------------------------------------
% EC2: Declipping
%
% StateFluent is declipped in [IntervalStartTime, IntervalEndTime)
% when an event initiates it inside that interval.
% ------------------------------------------------------------

declipped(IntervalStartTime, StateFluent, IntervalEndTime) :-
    timepoint(EffectTime),
    timepoint(IntervalStartTime),
    timepoint(IntervalEndTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, EffectTime),
    initiates(EventName, StateFluent, EffectTime),
    IntervalStartTime <= EffectTime,
    EffectTime < IntervalEndTime.

% ------------------------------------------------------------
% EC3: Stopped-in
%
% StateFluent is stopped strictly inside the interval.
% ------------------------------------------------------------

stopped_in(IntervalStartTime, StateFluent, IntervalEndTime) :-
    timepoint(EffectTime),
    timepoint(IntervalStartTime),
    timepoint(IntervalEndTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, EffectTime),
    terminates(EventName, StateFluent, EffectTime),
    IntervalStartTime < EffectTime,
    EffectTime < IntervalEndTime.

% ------------------------------------------------------------
% EC4: Started-in
%
% StateFluent is started strictly inside the interval.
% ------------------------------------------------------------

started_in(IntervalStartTime, StateFluent, IntervalEndTime) :-
    timepoint(EffectTime),
    timepoint(IntervalStartTime),
    timepoint(IntervalEndTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, EffectTime),
    initiates(EventName, StateFluent, EffectTime),
    IntervalStartTime < EffectTime,
    EffectTime < IntervalEndTime.

% ------------------------------------------------------------
% EC5: Trajectory
%
% An initiation of SourceFluent can create ResultFluent after Delay.
% ------------------------------------------------------------

holds_at(ResultFluent, EventTime + Delay) :-
    timepoint(EventTime),
    timepoint(Delay),
    fluent(SourceFluent),
    fluent(ResultFluent),
    event(EventName),
    happens(EventName, EventTime),
    initiates(EventName, SourceFluent, EventTime),
    trajectory(SourceFluent, EventTime, ResultFluent, Delay),
    0 < Delay,
    not stopped_in(EventTime, SourceFluent, EventTime + Delay).

% ------------------------------------------------------------
% EC6: Anti-trajectory
%
% A termination of SourceFluent can create ResultFluent after Delay.
% ------------------------------------------------------------

holds_at(ResultFluent, EventTime + Delay) :-
    timepoint(EventTime),
    timepoint(Delay),
    fluent(SourceFluent),
    fluent(ResultFluent),
    event(EventName),
    happens(EventName, EventTime),
    terminates(EventName, SourceFluent, EventTime),
    anti_trajectory(SourceFluent, EventTime, ResultFluent, Delay),
    0 < Delay,
    not started_in(EventTime, SourceFluent, EventTime + Delay).

% ------------------------------------------------------------
% EC7: Released-at propagation
% ------------------------------------------------------------

released_at_between(IntervalStartTime, StateFluent, IntervalEndTime) :-
    timepoint(ReleaseTime),
    timepoint(IntervalStartTime),
    timepoint(IntervalEndTime),
    fluent(StateFluent),
    released_at(StateFluent, ReleaseTime),
    IntervalStartTime < ReleaseTime,
    ReleaseTime <= IntervalEndTime.

% ------------------------------------------------------------
% EC8: Released-between
% ------------------------------------------------------------

released_between(IntervalStartTime, StateFluent, IntervalEndTime) :-
    timepoint(ReleaseTime),
    timepoint(IntervalStartTime),
    timepoint(IntervalEndTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, ReleaseTime),
    releases(EventName, StateFluent, ReleaseTime),
    IntervalStartTime <= ReleaseTime,
    ReleaseTime < IntervalEndTime.

% ------------------------------------------------------------
% EC9: Positive persistence
% ------------------------------------------------------------

holds_at(StateFluent, LaterTime) :-
    timepoint(EarlierTime),
    timepoint(LaterTime),
    fluent(StateFluent),
    holds_at(StateFluent, EarlierTime),
    EarlierTime < LaterTime,
    not released_at_between(EarlierTime, StateFluent, LaterTime),
    not clipped(EarlierTime, StateFluent, LaterTime).

% ------------------------------------------------------------
% EC10: Negative persistence
% ------------------------------------------------------------

-holds_at(StateFluent, LaterTime) :-
    timepoint(EarlierTime),
    timepoint(LaterTime),
    fluent(StateFluent),
    -holds_at(StateFluent, EarlierTime),
    EarlierTime < LaterTime,
    not released_at_between(EarlierTime, StateFluent, LaterTime),
    not declipped(EarlierTime, StateFluent, LaterTime).

% ------------------------------------------------------------
% EC11: Release persistence
% ------------------------------------------------------------

released_at(StateFluent, LaterTime) :-
    timepoint(EarlierTime),
    timepoint(LaterTime),
    fluent(StateFluent),
    released_at(StateFluent, EarlierTime),
    EarlierTime < LaterTime,
    not clipped(EarlierTime, StateFluent, LaterTime),
    not declipped(EarlierTime, StateFluent, LaterTime).

% ------------------------------------------------------------
% EC12: Non-release persistence
% ------------------------------------------------------------

-released_at(StateFluent, LaterTime) :-
    timepoint(EarlierTime),
    timepoint(LaterTime),
    fluent(StateFluent),
    -released_at(StateFluent, EarlierTime),
    EarlierTime < LaterTime,
    not released_between(EarlierTime, StateFluent, LaterTime).

% ------------------------------------------------------------
% EC13: Released-in
% ------------------------------------------------------------

released_in(IntervalStartTime, StateFluent, IntervalEndTime) :-
    timepoint(ReleaseTime),
    timepoint(IntervalStartTime),
    timepoint(IntervalEndTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, ReleaseTime),
    releases(EventName, StateFluent, ReleaseTime),
    IntervalStartTime < ReleaseTime,
    ReleaseTime < IntervalEndTime.

% ------------------------------------------------------------
% EC14: Initiation effect
%
% Effects initiated at EventTime become available strictly after
% EventTime.
% ------------------------------------------------------------

holds_at(StateFluent, LaterTime) :-
    timepoint(EventTime),
    timepoint(LaterTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, EventTime),
    initiates(EventName, StateFluent, EventTime),
    EventTime < LaterTime,
    not stopped_in(EventTime, StateFluent, LaterTime),
    not released_in(EventTime, StateFluent, LaterTime).

% ------------------------------------------------------------
% EC15: Termination effect
% ------------------------------------------------------------

-holds_at(StateFluent, LaterTime) :-
    timepoint(EventTime),
    timepoint(LaterTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, EventTime),
    terminates(EventName, StateFluent, EventTime),
    EventTime < LaterTime,
    not started_in(EventTime, StateFluent, LaterTime),
    not released_in(EventTime, StateFluent, LaterTime).

% ------------------------------------------------------------
% EC16: Release effect
% ------------------------------------------------------------

released_at(StateFluent, LaterTime) :-
    timepoint(EventTime),
    timepoint(LaterTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, EventTime),
    releases(EventName, StateFluent, EventTime),
    EventTime < LaterTime,
    not stopped_in(EventTime, StateFluent, LaterTime),
    not started_in(EventTime, StateFluent, LaterTime).

% ------------------------------------------------------------
% EC17A: End release through initiation
% ------------------------------------------------------------

-released_at(StateFluent, LaterTime) :-
    timepoint(EventTime),
    timepoint(LaterTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, EventTime),
    initiates(EventName, StateFluent, EventTime),
    EventTime < LaterTime,
    not released_in(EventTime, StateFluent, LaterTime).

% ------------------------------------------------------------
% EC17B: End release through termination
% ------------------------------------------------------------

-released_at(StateFluent, LaterTime) :-
    timepoint(EventTime),
    timepoint(LaterTime),
    fluent(StateFluent),
    event(EventName),
    happens(EventName, EventTime),
    terminates(EventName, StateFluent, EventTime),
    EventTime < LaterTime,
    not released_in(EventTime, StateFluent, LaterTime).

% ============================================================
% §3 EEC EXECUTABILITY EXTENSION
% ============================================================

% ------------------------------------------------------------
% EX1: Positive-precondition satisfaction
% ------------------------------------------------------------

precondition_satisfied(EventName, RequiredFluent, CandidateTime) :-
    event(EventName),
    fluent(RequiredFluent),
    timepoint(CandidateTime),
    precondition(EventName, RequiredFluent, CandidateTime),
    holds_at(RequiredFluent, CandidateTime).

% ------------------------------------------------------------
% EX2: Negative-precondition satisfaction
% ------------------------------------------------------------

negative_precondition_satisfied(EventName, ForbiddenFluent, CandidateTime) :-
    event(EventName),
    fluent(ForbiddenFluent),
    timepoint(CandidateTime),
    negative_precondition(EventName, ForbiddenFluent, CandidateTime),
    not holds_at(ForbiddenFluent, CandidateTime).

% ------------------------------------------------------------
% EX3: Positive-precondition violation
% ------------------------------------------------------------

precondition_violated(EventName, RequiredFluent, CandidateTime) :-
    event(EventName),
    fluent(RequiredFluent),
    timepoint(CandidateTime),
    precondition(EventName, RequiredFluent, CandidateTime),
    not holds_at(RequiredFluent, CandidateTime).

% ------------------------------------------------------------
% EX4: Negative-precondition violation
% ------------------------------------------------------------

negative_precondition_violated(EventName, ForbiddenFluent, CandidateTime) :-
    event(EventName),
    fluent(ForbiddenFluent),
    timepoint(CandidateTime),
    negative_precondition(EventName, ForbiddenFluent, CandidateTime),
    holds_at(ForbiddenFluent, CandidateTime).

% ------------------------------------------------------------
% EX5A: Generic Event Calculus executability
%
% Used when external HTP context enforcement is not active.
% ------------------------------------------------------------

can_happen(EventName, CandidateTime) :-
    event(EventName),
    timepoint(CandidateTime),
    not external_htp_domain,
    not cannot_happen(EventName, CandidateTime).

% ------------------------------------------------------------
% EX5B: HTP-constrained executability
%
% Used when external_htp_domain. is active. A primitive event is
% executable only when the external HTP domain marks it eligible.
% ------------------------------------------------------------

can_happen(PrimitiveAction, CandidateTime) :-
    event(PrimitiveAction),
    primitive(PrimitiveAction),
    timepoint(CandidateTime),
    external_htp_domain,
    htp_eligible(PrimitiveAction, CandidateTime),
    not cannot_happen(PrimitiveAction, CandidateTime).

% ------------------------------------------------------------
% EX6A: Positive-precondition non-executability
% ------------------------------------------------------------

cannot_happen(EventName, CandidateTime) :-
    event(EventName),
    fluent(RequiredFluent),
    timepoint(CandidateTime),
    precondition_violated(EventName, RequiredFluent, CandidateTime).

% ------------------------------------------------------------
% EX6B: Negative-precondition non-executability
% ------------------------------------------------------------

cannot_happen(EventName, CandidateTime) :-
    event(EventName),
    fluent(ForbiddenFluent),
    timepoint(CandidateTime),
    negative_precondition_violated(
        EventName,
        ForbiddenFluent,
        CandidateTime
    ).

% ------------------------------------------------------------
% EX6C: External HTP context non-executability
%
% A primitive event is blocked when it is outside the selected
% decomposition tree at the candidate time.
% ------------------------------------------------------------

cannot_happen(PrimitiveAction, CandidateTime) :-
    event(PrimitiveAction),
    primitive(PrimitiveAction),
    timepoint(CandidateTime),
    external_htp_domain,
    not htp_eligible(PrimitiveAction, CandidateTime).

% ------------------------------------------------------------
% EX7: Narrative consistency
% ------------------------------------------------------------

:- happens(EventName, OccurrenceTime),
   cannot_happen(EventName, OccurrenceTime),
   event(EventName),
   timepoint(OccurrenceTime).

% ============================================================
% §4 INTEGRATION BRIDGES
% ============================================================

% ------------------------------------------------------------
% B1: Primitive actions are Event Calculus events.
% ------------------------------------------------------------

event(PrimitiveAction) :-
    primitive(PrimitiveAction).

% ------------------------------------------------------------
% B2: Occurrence / event unification.
% ------------------------------------------------------------

happens(PrimitiveAction, OccurrenceTime) :-
    occurs(PrimitiveAction, OccurrenceTime),
    primitive(PrimitiveAction),
    timepoint(OccurrenceTime).

occurs(PrimitiveAction, OccurrenceTime) :-
    happens(PrimitiveAction, OccurrenceTime),
    primitive(PrimitiveAction),
    timepoint(OccurrenceTime).

% ------------------------------------------------------------
% B3: State unification.
% ------------------------------------------------------------

holds(StateFluent, QueryTime) :-
    holds_at(StateFluent, QueryTime),
    fluent(StateFluent),
    timepoint(QueryTime).

holds_at(StateFluent, QueryTime) :-
    holds(StateFluent, QueryTime),
    fluent(StateFluent),
    timepoint(QueryTime).

% ------------------------------------------------------------
% B4: Static add effects become EEC initiations.
% ------------------------------------------------------------

initiates(PrimitiveAction, AddedFluent, EventTime) :-
    add_effect(PrimitiveAction, AddedFluent),
    primitive(PrimitiveAction),
    fluent(AddedFluent),
    timepoint(EventTime).

% ------------------------------------------------------------
% B5: Static delete effects become EEC terminations.
% ------------------------------------------------------------

terminates(PrimitiveAction, DeletedFluent, EventTime) :-
    del_effect(PrimitiveAction, DeletedFluent),
    primitive(PrimitiveAction),
    fluent(DeletedFluent),
    timepoint(EventTime).

% ------------------------------------------------------------
% B6: Static positive preconditions become EEC preconditions.
% ------------------------------------------------------------

precondition(PrimitiveAction, RequiredFluent, QueryTime) :-
    precondition_static(PrimitiveAction, RequiredFluent),
    primitive(PrimitiveAction),
    fluent(RequiredFluent),
    timepoint(QueryTime).

% ------------------------------------------------------------
% B7: Static negative preconditions become EEC preconditions.
% ------------------------------------------------------------

negative_precondition(PrimitiveAction, ForbiddenFluent, QueryTime) :-
    negative_precondition_static(PrimitiveAction, ForbiddenFluent),
    primitive(PrimitiveAction),
    fluent(ForbiddenFluent),
    timepoint(QueryTime).

% ------------------------------------------------------------
% B8: Method preconditions apply to their compound tasks.
% ------------------------------------------------------------

precondition(CompoundTask, RequiredFluent, QueryTime) :-
    method_precondition(MethodName, RequiredFluent),
    method(MethodName, CompoundTask),
    compound(CompoundTask),
    fluent(RequiredFluent),
    timepoint(QueryTime).

% ------------------------------------------------------------
% B9: Positive initial-state bridge.
% ------------------------------------------------------------

holds(StateFluent, 0) :-
    holds_at(StateFluent, 0),
    fluent(StateFluent).

% ------------------------------------------------------------
% B10: Initial-polarity consistency.
% ------------------------------------------------------------

:- -holds_at(StateFluent, 0),
   holds(StateFluent, 0),
   fluent(StateFluent).

% ------------------------------------------------------------
% B11: Goal evaluation.
% ------------------------------------------------------------

goal_satisfied(GoalFluent, GoalEvaluationTime) :-
    goal(GoalFluent, GoalEvaluationTime),
    holds_at(GoalFluent, GoalEvaluationTime),
    fluent(GoalFluent),
    timepoint(GoalEvaluationTime).

goal_violated(GoalFluent, GoalEvaluationTime) :-
    goal(GoalFluent, GoalEvaluationTime),
    fluent(GoalFluent),
    timepoint(GoalEvaluationTime),
    not holds_at(GoalFluent, GoalEvaluationTime).

% ============================================================
% §5 EXTERNAL HTP EXECUTION CONTRACT
% ============================================================

% When external HTP enforcement is active, every scheduled
% primitive action must be an eligible leaf of the selected HTP
% decomposition at its occurrence time.

:- external_htp_domain,
   occurs(PrimitiveAction, OccurrenceTime),
   primitive(PrimitiveAction),
   timepoint(OccurrenceTime),
   not htp_eligible(PrimitiveAction, OccurrenceTime).

% Every scheduled primitive action must also be EEC-executable.

:- external_htp_domain,
   occurs(PrimitiveAction, OccurrenceTime),
   primitive(PrimitiveAction),
   timepoint(OccurrenceTime),
   not can_happen(PrimitiveAction, OccurrenceTime).

% ============================================================
% §6 DERIVED QUERY PREDICATES
% ============================================================

% ------------------------------------------------------------
% Q1: Full primitive-action plan trace.
% ------------------------------------------------------------

plan_step(PrimitiveAction, OccurrenceTime) :-
    occurs(PrimitiveAction, OccurrenceTime),
    primitive(PrimitiveAction),
    timepoint(OccurrenceTime).

plan_step(PrimitiveAction, OccurrenceTime) :-
    happens(PrimitiveAction, OccurrenceTime),
    primitive(PrimitiveAction),
    timepoint(OccurrenceTime).

% ------------------------------------------------------------
% Q2: Compound tasks active during their applied span.
% ------------------------------------------------------------

active_task(CompoundTask, QueryTime) :-
    compound(CompoundTask),
    task_span(CompoundTask, TaskStartTime, TaskEndTime),
    timepoint(QueryTime),
    TaskStartTime <= QueryTime,
    QueryTime <= TaskEndTime.

% ------------------------------------------------------------
% Q3: Applied decomposition trace.
% ------------------------------------------------------------

decomposed_by(
    CompoundTask,
    MethodName,
    MethodStartTime,
    MethodEndTime
) :-
    compound(CompoundTask),
    method(MethodName, CompoundTask),
    applied(MethodName, MethodStartTime, MethodEndTime),
    timepoint(MethodStartTime),
    timepoint(MethodEndTime).

% ------------------------------------------------------------
% Q4: HTP-selected and EEC-executable primitive action.
% ------------------------------------------------------------

all_preconditions_met(PrimitiveAction, CandidateTime) :-
    event(PrimitiveAction),
    primitive(PrimitiveAction),
    htp_eligible(PrimitiveAction, CandidateTime),
    can_happen(PrimitiveAction, CandidateTime),
    timepoint(CandidateTime).

% ------------------------------------------------------------
% Q5: Plan validity.
% ------------------------------------------------------------

plan_invalid :-
    goal_violated(GoalFluent, GoalEvaluationTime),
    fluent(GoalFluent),
    timepoint(GoalEvaluationTime).

plan_invalid :-
    happens(EventName, OccurrenceTime),
    cannot_happen(EventName, OccurrenceTime),
    event(EventName),
    timepoint(OccurrenceTime).

root_task_has_span(RootCompoundTask, RootTaskStartTime) :-
    root_task(RootCompoundTask, RootTaskStartTime),
    compound(RootCompoundTask),
    task_span(
        RootCompoundTask,
        RootTaskStartTime,
        RootTaskEndTime
    ),
    timepoint(RootTaskStartTime),
    timepoint(RootTaskEndTime).

plan_invalid :-
    external_htp_domain,
    root_task(RootCompoundTask, RootTaskStartTime),
    compound(RootCompoundTask),
    timepoint(RootTaskStartTime),
    not root_task_has_span(RootCompoundTask, RootTaskStartTime).

plan_valid :-
    not plan_invalid.

% ------------------------------------------------------------
% Q6: HTP-eligible but EEC-blocked primitive action.
% ------------------------------------------------------------

eligible_but_blocked(PrimitiveAction, CandidateTime) :-
    htp_eligible(PrimitiveAction, CandidateTime),
    primitive(PrimitiveAction),
    timepoint(CandidateTime),
    cannot_happen(PrimitiveAction, CandidateTime).

% ------------------------------------------------------------
% Q7: Primitive action excluded from selected decomposition.
% ------------------------------------------------------------

outside_selected_decomposition(PrimitiveAction, CandidateTime) :-
    event(PrimitiveAction),
    primitive(PrimitiveAction),
    timepoint(CandidateTime),
    external_htp_domain,
    not htp_eligible(PrimitiveAction, CandidateTime).

% ------------------------------------------------------------
% Q8: Fluent missing an initial polarity.
% ------------------------------------------------------------

fluent_no_initial_polarity(StateFluent) :-
    fluent(StateFluent),
    not holds_at(StateFluent, 0),
    not -holds_at(StateFluent, 0).

% ------------------------------------------------------------
% Q9: Concurrent delete / positive-precondition conflict.
% ------------------------------------------------------------

concurrent_del_pre_conflict(
    DeletingAction,
    PreconditionUsingAction,
    ConflictingFluent,
    OccurrenceTime
) :-
    occurs(DeletingAction, OccurrenceTime),
    occurs(PreconditionUsingAction, OccurrenceTime),
    primitive(DeletingAction),
    primitive(PreconditionUsingAction),
    del_effect(DeletingAction, ConflictingFluent),
    precondition_static(PreconditionUsingAction, ConflictingFluent),
    fluent(ConflictingFluent),
    timepoint(OccurrenceTime),
    DeletingAction != PreconditionUsingAction.

% ------------------------------------------------------------
% Q10: Same-step initiator / precondition diagnostic.
% ------------------------------------------------------------

precondition_same_step_as_initiator(
    PreconditionUsingAction,
    InitiatingAction,
    RequiredFluent,
    OccurrenceTime
) :-
    precondition_static(PreconditionUsingAction, RequiredFluent),
    add_effect(InitiatingAction, RequiredFluent),
    occurs(PreconditionUsingAction, OccurrenceTime),
    occurs(InitiatingAction, OccurrenceTime),
    primitive(PreconditionUsingAction),
    primitive(InitiatingAction),
    fluent(RequiredFluent),
    timepoint(OccurrenceTime),
    PreconditionUsingAction != InitiatingAction,
    not holds_at(RequiredFluent, OccurrenceTime).

% ============================================================
% §7 RECOMMENDED SHOW DIRECTIVES
% ============================================================

% #show plan_step/2.
% #show decomposed_by/4.
% #show active_task/2.
% #show goal_satisfied/2.
% #show goal_violated/2.
% #show plan_valid/0.
% #show plan_invalid/0.
%
% #show holds_at/2.
% #show clipped/3.
% #show initiates/3.
% #show terminates/3.
%
% #show precondition_violated/3.
% #show negative_precondition_violated/3.
% #show htp_eligible/2.
% #show can_happen/2.
% #show cannot_happen/2.
% #show eligible_but_blocked/2.
% #show outside_selected_decomposition/2.
% #show all_preconditions_met/2.
%
% #show selected/3.
% #show applied/3.
% #show task_span/3.
% #show task_start/2.
%
% #show fluent_no_initial_polarity/1.
% #show concurrent_del_pre_conflict/4.
% #show precondition_same_step_as_initiator/4.

% ============================================================
% End of eec_htp_integration_v5.lp
% ============================================================