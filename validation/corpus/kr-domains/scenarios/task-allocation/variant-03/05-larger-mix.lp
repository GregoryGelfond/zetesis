% Scenario 05: Larger 4-task, 3-agent instance with one binding group.
% @expect sat
% @cost { 6 }
% @count optimal 1
% @optimal { assigned_to(a2,t1), assigned_to(a2,t2), assigned_to(a3,t3), assigned_to(a3,t4), assigned_cost(a2,t1,3), assigned_cost(a2,t2,1), assigned_cost(a3,t3,1), assigned_cost(a3,t4,1) }
%
% Ungrouped, each task takes its cheapest agent for total 4. in_group(t1,g1) and
% in_group(t2,g1) force t1 and t2 to share; the cheapest joint choice is a2
% (3 + 1 = 4) over a1 (6) or a3 (8). t3 and t4 stay free with a3. Total 6, up from
% the ungrouped 4.

#include "../../../encodings/task-allocation/variant-03.lp".

task(t1; t2; t3; t4).
agent(a1; a2; a3).

cost_of(
    a1,t1,1;
    a2,t1,3;
    a3,t1,5;
    a1,t2,5;
    a2,t2,1;
    a3,t2,3;
    a1,t3,3;
    a2,t3,5;
    a3,t3,1;
    a1,t4,5;
    a2,t4,3;
    a3,t4,1
).

in_group(
    t1,g1;
    t2,g1
).
