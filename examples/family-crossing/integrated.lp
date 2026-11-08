% ============================================================
% family_crossing_mode_b_generative_planning.lp
%
% Mode B: generative planning.
%
% No ground selected/3, applied/3, method_assignment/3,
% applied_explicit/3, or occurs/2 scenario facts are supplied.
% ============================================================

#include "family_crossing_complete_domain.lp".

% ------------------------------------------------------------
% Root-method selection: bounded plan-length alternatives
% ------------------------------------------------------------

1 {
    selected(m_5, cross_all, 0);
    selected(m_7, cross_all, 0);
    selected(m_9, cross_all, 0)
} 1.

applied(M, 0, horizon) :-
    selected(M, cross_all, 0),
    root_method(M).

% ------------------------------------------------------------
% Exactly one primitive crossing method per active stage
% ------------------------------------------------------------

1 {
    selected(use(A, T), stage(T), T) : primitive(A)
} 1 :-
    active_stage(T),
    plan_slot(T).

applied(use(A, T), T, TNext) :-
    selected(use(A, T), stage(T), T),
    primitive(A),
    transition(T, TNext).

% Every selected stage leaf produces one occurrence.
occurs(A, T) :-
    selected(use(A, T), stage(T), T),
    primitive(A),
    active_stage(T),
    plan_slot(T).

% No inactive stage may carry an occurrence.
:- occurs(A, T),
   primitive(A),
   plan_slot(T),
   not active_stage(T).

% A selected root decomposition must have the required task span.
:- selected(M, cross_all, 0),
   root_method(M),
   not applied(M, 0, horizon).

% Minimize total crossing time.
#minimize {
    Cost@1, T, A :
        occurs(A, T),
        primitive(A),
        action_cost(A, Cost),
        timepoint(T)
}.

% #show selected/3.
% #show applied/3.
#show occurs/2.
% #show htp_eligible/2.
#show can_happen/2.
#show total_time_at/2.
#show goal_satisfied/2.
#show plan_valid/0.