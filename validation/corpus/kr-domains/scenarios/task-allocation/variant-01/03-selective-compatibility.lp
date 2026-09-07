% Scenario 03: Selective compatibility, tasks with different candidate sets.
% @expect sat
% @cost { 10 }
% @count optimal 1
% @optimal { assigned_to(a1,t3), assigned_to(a2,t1), assigned_to(a3,t2), assigned_cost(a1,t3,5), assigned_cost(a2,t1,2), assigned_cost(a3,t2,3) }
%
% Each task has only a subset of agents with a defined cost (so compatible_with/2
% is partial): t1 takes a2 (2), t2 takes a3 (3), and t3 has only a1 (5), for a
% total of 10.

#include "../../../encodings/task-allocation/variant-01.lp".

task(t1; t2; t3).

agent(a1; a2; a3).

cost_of(
    a1,t1,4;
    a2,t1,2
).

cost_of(
    a2,t2,7;
    a3,t2,3
).

cost_of(a1,t3,5).
