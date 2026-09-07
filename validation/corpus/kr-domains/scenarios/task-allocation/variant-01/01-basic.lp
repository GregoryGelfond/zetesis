% Scenario 01: Basic 2-task, 2-agent assignment.
% @expect sat
% @cost { 5 }
% @count optimal 1
% @optimal { assigned_to(a1,t1), assigned_to(a2,t2), assigned_cost(a1,t1,3), assigned_cost(a2,t2,2) }
%
% Both agents can take both tasks. The cheapest agent per task is a1 for t1
% (cost 3) and a2 for t2 (cost 2), for a total of 5.

#include "../../../encodings/task-allocation/variant-01.lp".

task(t1; t2).

agent(a1; a2).

cost_of(
    a1,t1,3;
    a2,t1,5;
    a1,t2,4;
    a2,t2,2
).
