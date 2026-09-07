% Scenario 05: All four mechanisms active at once.
% @expect sat
% @cost { 5 }
% @cautious optimal { assigned_to(a1,t1), assigned_to(a1,t3), assigned_to(a2,t2), assigned_to(a2,t4), assigned_cost(a1,t1,1), assigned_cost(a1,t3,2), assigned_cost(a2,t2,1), assigned_cost(a2,t4,1) }
%
% Four tasks exercising grouping (t1 and t3 share an agent), precedence (t2 before
% t4), serialization (no same-agent overlap), and windows (all [0,10]). Cost-min:
% group {t1,t3} to a1 (1 + 2 = 3, against a2's 5), t2 to a2 (1), t4 to a2 (1), for
% a total of 5. a1 runs t1 and t3 (durations 2, 3); a2 runs t2 before t4
% (durations 2, 2); both fit [0,10] with the ordering respected.

#include "../../../encodings/task-allocation/variant-04.lp".

task(t1; t2; t3; t4).
agent(a1; a2).

cost_of(
    a1,t1,1;
    a2,t1,3;
    a1,t2,3;
    a2,t2,1;
    a1,t3,2;
    a2,t3,2;
    a1,t4,3;
    a2,t4,1
).

completion_time(
    a1,t1,2;
    a2,t1,2;
    a1,t2,2;
    a2,t2,2;
    a1,t3,3;
    a2,t3,3;
    a1,t4,2;
    a2,t4,2
).

time_window(
    t1,0,10;
    t2,0,10;
    t3,0,10;
    t4,0,10
).

in_group(
    t1,g1;
    t3,g1
).

before(t2,t4).
