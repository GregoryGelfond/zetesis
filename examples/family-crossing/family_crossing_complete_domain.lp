% ============================================================
% family_crossing_complete_domain.lp
%
% Complete EEC+HTP domain for the five-person bridge crossing.
% ============================================================

#include "eec_htp_integration_v5.lp".

#const horizon = 9.

% ------------------------------------------------------------
% Finite domains
% ------------------------------------------------------------

external_htp_domain.

timepoint(0..horizon).
step(0..horizon).

transition(T, TNext) :-
    timepoint(T),
    timepoint(TNext),
    TNext = T + 1.

plan_slot(T) :-
    timepoint(T),
    T < horizon.

person(p1).
person(p2).
person(p3).
person(p4).
person(p5).

% Crossing speeds in seconds.
speed(p1, 1).
speed(p2, 6).
speed(p3, 3).
speed(p4, 8).
speed(p5, 12).

side(left).
side(right).

capacity(2).

% Ten unordered two-person groups.
pair(p1, p2).
pair(p1, p3).
pair(p1, p4).
pair(p1, p5).
pair(p2, p3).
pair(p2, p4).
pair(p2, p5).
pair(p3, p4).
pair(p3, p5).
pair(p4, p5).

% ------------------------------------------------------------
% Fluents and initial state
% ------------------------------------------------------------

fluent(at(P, S)) :-
    person(P),
    side(S).

fluent(torch(S)) :-
    side(S).

holds_at(at(P, left), 0) :-
    person(P).

holds_at(torch(left), 0).

% A person and the torch cannot be on both sides simultaneously.
:- holds_at(at(P, left), T),
   holds_at(at(P, right), T),
   person(P),
   timepoint(T).

:- holds_at(torch(left), T),
   holds_at(torch(right), T),
   timepoint(T).

% ------------------------------------------------------------
% Primitive crossing actions
% ------------------------------------------------------------

primitive(cross_lr(P)) :-
    person(P).

primitive(cross_rl(P)) :-
    person(P).

primitive(cross_lr(P, Q)) :-
    pair(P, Q).

primitive(cross_rl(P, Q)) :-
    pair(P, Q).

% ------------------------------------------------------------
% Crossing costs: maximum speed in the group
% ------------------------------------------------------------

action_cost(cross_lr(P), Cost) :-
    speed(P, Cost).

action_cost(cross_rl(P), Cost) :-
    speed(P, Cost).

action_cost(cross_lr(P, Q), CostP) :-
    pair(P, Q),
    speed(P, CostP),
    speed(Q, CostQ),
    CostP >= CostQ.

action_cost(cross_lr(P, Q), CostQ) :-
    pair(P, Q),
    speed(P, CostP),
    speed(Q, CostQ),
    CostQ > CostP.

action_cost(cross_rl(P, Q), CostP) :-
    pair(P, Q),
    speed(P, CostP),
    speed(Q, CostQ),
    CostP >= CostQ.

action_cost(cross_rl(P, Q), CostQ) :-
    pair(P, Q),
    speed(P, CostP),
    speed(Q, CostQ),
    CostQ > CostP.

% ------------------------------------------------------------
% EEC preconditions and state effects
% ------------------------------------------------------------

% Left-to-right singleton crossing.
precondition_static(cross_lr(P), at(P, left)) :-
    person(P).

precondition_static(cross_lr(P), torch(left)) :-
    person(P).

add_effect(cross_lr(P), at(P, right)) :-
    person(P).

add_effect(cross_lr(P), torch(right)) :-
    person(P).

del_effect(cross_lr(P), at(P, left)) :-
    person(P).

del_effect(cross_lr(P), torch(left)) :-
    person(P).

% Right-to-left singleton crossing.
precondition_static(cross_rl(P), at(P, right)) :-
    person(P).

precondition_static(cross_rl(P), torch(right)) :-
    person(P).

add_effect(cross_rl(P), at(P, left)) :-
    person(P).

add_effect(cross_rl(P), torch(left)) :-
    person(P).

del_effect(cross_rl(P), at(P, right)) :-
    person(P).

del_effect(cross_rl(P), torch(right)) :-
    person(P).

% Left-to-right pair crossing.
precondition_static(cross_lr(P, Q), at(P, left)) :-
    pair(P, Q).

precondition_static(cross_lr(P, Q), at(Q, left)) :-
    pair(P, Q).

precondition_static(cross_lr(P, Q), torch(left)) :-
    pair(P, Q).

add_effect(cross_lr(P, Q), at(P, right)) :-
    pair(P, Q).

add_effect(cross_lr(P, Q), at(Q, right)) :-
    pair(P, Q).

add_effect(cross_lr(P, Q), torch(right)) :-
    pair(P, Q).

del_effect(cross_lr(P, Q), at(P, left)) :-
    pair(P, Q).

del_effect(cross_lr(P, Q), at(Q, left)) :-
    pair(P, Q).

del_effect(cross_lr(P, Q), torch(left)) :-
    pair(P, Q).

% Right-to-left pair crossing.
precondition_static(cross_rl(P, Q), at(P, right)) :-
    pair(P, Q).

precondition_static(cross_rl(P, Q), at(Q, right)) :-
    pair(P, Q).

precondition_static(cross_rl(P, Q), torch(right)) :-
    pair(P, Q).

add_effect(cross_rl(P, Q), at(P, left)) :-
    pair(P, Q).

add_effect(cross_rl(P, Q), at(Q, left)) :-
    pair(P, Q).

add_effect(cross_rl(P, Q), torch(left)) :-
    pair(P, Q).

del_effect(cross_rl(P, Q), at(P, right)) :-
    pair(P, Q).

del_effect(cross_rl(P, Q), at(Q, right)) :-
    pair(P, Q).

del_effect(cross_rl(P, Q), torch(right)) :-
    pair(P, Q).

% ------------------------------------------------------------
% Goal state
% ------------------------------------------------------------

goal(at(P, right), horizon) :-
    person(P).

goal(torch(right), horizon).

:- goal(F, T),
   fluent(F),
   timepoint(T),
   not holds_at(F, T).

% ------------------------------------------------------------
% HTP task hierarchy
%
% cross_all
%   -> stage(0), stage(1), ..., stage(N-1)
%   -> one selected primitive crossing action per active stage
% ------------------------------------------------------------

compound(cross_all).

compound(stage(T)) :-
    plan_slot(T).

root_task(cross_all, 0).
task_start(cross_all, 0).

root_method(m_5).
root_method(m_7).
root_method(m_9).

method(m_5, cross_all).
method(m_7, cross_all).
method(m_9, cross_all).

slot_5(0..4).
slot_7(0..6).
slot_9(0..8).

subtask(m_5, stage(T), T) :-
    slot_5(T).

subtask(m_7, stage(T), T) :-
    slot_7(T).

subtask(m_9, stage(T), T) :-
    slot_9(T).

% Each stage has a candidate method for every primitive crossing.
method(use(A, T), stage(T)) :-
    primitive(A),
    plan_slot(T).

task_start(stage(T), T) :-
    plan_slot(T).

% The selected root method determines active stages.
active_stage(T) :-
    selected(m_5, cross_all, 0),
    slot_5(T).

active_stage(T) :-
    selected(m_7, cross_all, 0),
    slot_7(T).

active_stage(T) :-
    selected(m_9, cross_all, 0),
    slot_9(T).

% Root span includes the goal-evaluation timepoint.
task_span(cross_all, 0, horizon).

task_span(stage(T), T, TNext) :-
    active_stage(T),
    transition(T, TNext).

% A selected stage method exposes exactly its associated primitive leaf.
leaf_slot(use(A, T), A, T) :-
    method(use(A, T), stage(T)),
    primitive(A),
    plan_slot(T).

% HTP eligibility comes only from a selected and applied stage method.
htp_eligible(A, T) :-
    selected(use(A, T), stage(T), T),
    applied(use(A, T), T, TNext),
    leaf_slot(use(A, T), A, T),
    primitive(A),
    transition(T, TNext).

% Structural HTP consistency.
:- selected(M, stage(T), Start),
   method(M, stage(T)),
   not active_stage(T),
   timepoint(Start).

:- selected(M, stage(T), Start),
   method(M, stage(T)),
   active_stage(T),
   Start != T,
   timepoint(Start).

:- selected(M, stage(T), T),
   method(M, stage(T)),
   active_stage(T),
   transition(T, TNext),
   not applied(M, T, TNext).

% ------------------------------------------------------------
% Derived cumulative time
% ------------------------------------------------------------

occurs_at(T) :-
    occurs(A, T),
    primitive(A),
    timepoint(T).

idle_slot(T) :-
    plan_slot(T),
    not occurs_at(T).

total_time_at(0, 0).

total_time_at(TNext, TotalNext) :-
    transition(T, TNext),
    total_time_at(T, Total),
    occurs(A, T),
    primitive(A),
    action_cost(A, Cost),
    TotalNext = Total + Cost.

total_time_at(TNext, Total) :-
    transition(T, TNext),
    total_time_at(T, Total),
    idle_slot(T).

% #show occurs/2.
% #show selected/3.
% #show applied/3.
% #show htp_eligible/2.
% #show can_happen/2.
% #show total_time_at/2.
% #show goal_satisfied/2.
% #show plan_valid/0.
