% Scenario 05: Larger 4-task, 3-agent mix.
% @expect sat
% @cost { 9 7 }
% @count optimal 1
% @optimal { assigned_to(a1,t1), assigned_to(a2,t2), assigned_to(a3,t3), assigned_to(a3,t4), assigned_cost(a1,t1,2), assigned_cost(a2,t2,2), assigned_cost(a3,t3,2), assigned_cost(a3,t4,3) }
%
% Cost-cheapest per task is unique: t1 -> a1 (2), t2 -> a2 (2), t3 -> a3 (2),
% t4 -> a3 (3), total 9. The makespan tie-break does not bind, but its value is
% still reported: a3 takes t3 and t4, giving times a1 = 3, a2 = 4, a3 = 5 + 2 = 7,
% so makespan max(3, 4, 7) = 7.

#include "../../../encodings/task-allocation/variant-02.lp".

task(t1; t2; t3; t4).
agent(a1; a2; a3).

cost_of(
    a1,t1,2;
    a2,t1,3;
    a3,t1,5;
    a1,t2,4;
    a2,t2,2;
    a3,t2,4;
    a1,t3,3;
    a2,t3,5;
    a3,t3,2;
    a1,t4,5;
    a2,t4,4;
    a3,t4,3
).

completion_time(
    a1,t1,3;
    a2,t1,2;
    a3,t1,1;
    a1,t2,2;
    a2,t2,4;
    a3,t2,3;
    a1,t3,4;
    a2,t3,1;
    a3,t3,5;
    a1,t4,1;
    a2,t4,3;
    a3,t4,2
).
