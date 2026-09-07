% Scenario 03: Grouped and ungrouped tasks in one instance.
% @expect sat
% @cost { 8 }
% @count optimal 2
% @cautious optimal { assigned_to(a2,t3), assigned_cost(a2,t3,2) }
%
% t1 and t2 share group g1, so one agent takes both (best total 6, either agent).
% t3 is ungrouped and free to take its own cheapest agent, a2 at cost 2. Total 8.
% The two optima differ on the group's agent but both pin t3 to a2.

#include "../../../encodings/task-allocation/variant-03.lp".

task(t1; t2; t3).
agent(a1; a2).

cost_of(
    a1,t1,1;
    a2,t1,5;
    a1,t2,5;
    a2,t2,1;
    a1,t3,3;
    a2,t3,2
).

in_group(
    t1,g1;
    t2,g1
).
% t3 is ungrouped.
