% Scenario 03: Cost dominance, agent reuse wins despite a bad makespan.
% @expect sat
% @cost { 3 15 }
% @count optimal 1
% @optimal { assigned_to(a1,t1), assigned_to(a1,t2), assigned_to(a1,t3), assigned_cost(a1,t1,1), assigned_cost(a1,t2,1), assigned_cost(a1,t3,1) }
%
% a1 is strictly cheaper for every task (1 against 3) but strictly slower (5
% against 1). Cost-min is unique: a1 takes all three at total cost 3, makespan 15
% (a1 runs them sequentially). A better makespan would require moving a task to a2
% at a higher cost, which the lexicographic order forbids.

#include "../../../encodings/task-allocation/variant-02.lp".

task(t1; t2; t3).
agent(a1; a2).

cost_of(
    a1,t1,1;
    a2,t1,3;
    a1,t2,1;
    a2,t2,3;
    a1,t3,1;
    a2,t3,3
).

completion_time(
    a1,t1,5;
    a2,t1,1;
    a1,t2,5;
    a2,t2,1;
    a1,t3,5;
    a2,t3,1
).
