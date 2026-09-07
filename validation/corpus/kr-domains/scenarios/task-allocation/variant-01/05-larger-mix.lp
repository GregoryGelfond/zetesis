% Scenario 05: Larger mix, 4 tasks, 3 agents, fully compatible.
% @expect sat
% @cost { 9 }
% @count optimal 1
% @optimal { assigned_to(a1,t2), assigned_to(a2,t3), assigned_to(a3,t1), assigned_to(a3,t4), assigned_cost(a1,t2,3), assigned_cost(a2,t3,1), assigned_cost(a3,t1,2), assigned_cost(a3,t4,3) }
%
% Each task takes its cheapest agent independently (no exclusivity): t1 -> a3 (2),
% t2 -> a1 (3), t3 -> a2 (1), t4 -> a3 (3), for a total of 9. a3 serves two tasks.

#include "../../../encodings/task-allocation/variant-01.lp".

task(t1; t2; t3; t4).

agent(a1; a2; a3).

cost_of(
    a1,t1,4;
    a2,t1,6;
    a3,t1,2;
    a1,t2,3;
    a2,t2,5;
    a3,t2,7;
    a1,t3,8;
    a2,t3,1;
    a3,t3,6;
    a1,t4,5;
    a2,t4,9;
    a3,t4,3
).
