% Scenario 02: Agent reuse, one agent takes every task.
% @expect sat
% @cost { 6 }
% @count optimal 1
% @optimal { assigned_to(a1,t1), assigned_to(a1,t2), assigned_to(a1,t3), assigned_cost(a1,t1,1), assigned_cost(a1,t2,2), assigned_cost(a1,t3,3) }
%
% a1 is strictly cheapest for all three tasks (1, 2, 3 against a2's flat 5).
% variant-01 imposes no per-agent capacity, so the optimum gives all three to a1,
% for a total of 6. This is what distinguishes the encoding from bipartite
% matching, where each agent would take at most one task.

#include "../../../encodings/task-allocation/variant-01.lp".

task(t1; t2; t3).

agent(a1; a2).

cost_of(
    a1,t1,1;
    a2,t1,5;
    a1,t2,2;
    a2,t2,5;
    a1,t3,3;
    a2,t3,5
).
